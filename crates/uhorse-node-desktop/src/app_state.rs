use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use chrono::Utc;
use reqwest::{header, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;
use uhorse_node_runtime::{
    ConnectionState, Node, NodeConfig, NodeError, NodeResult, NotificationEventKind,
    VersionManager, Workspace,
};

use crate::config_store::{
    current_computer_name, ConfigStore, DesktopConfig, DesktopPreferencesConfig,
};
use crate::dto::{
    ApiResponse, CancelAccountPairingRequest, DefaultSettingsDto, DesktopAccountStatusDto,
    DesktopCapabilityStatusDto, DesktopCheckpointDto, DesktopConnectionDiagnosticsDto,
    DesktopConnectionRecoveryResultDto, DesktopHeartbeatDto, DesktopLifecycleState,
    DesktopLogEntryDto, DesktopLogLevel, DesktopMetricsDto, DesktopNodeStatusDto,
    DesktopPairingRequestDto, DesktopSettingsDto, DesktopVersionEntryDto, DesktopVersionSummaryDto,
    DesktopWorkspaceStatusDto, StartAccountPairingRequest, WorkspaceValidationDto,
};

const DESKTOP_LAUNCH_AGENT_ID: &str = "com.uhorse.node-desktop";
const DEFAULT_DESKTOP_LISTEN: &str = "127.0.0.1:8757";
const DEFAULT_DEVICE_TYPE: &str = "desktop";
const HUB_API_AUTH_HEADER_PREFIX: &str = "Bearer ";

#[derive(Clone)]
pub struct DesktopAppState {
    inner: Arc<RwLock<DesktopApp>>,
}

struct DesktopApp {
    config_store: ConfigStore,
    config: NodeConfig,
    desktop: DesktopPreferencesConfig,
    lifecycle_state: DesktopLifecycleState,
    node: Option<Node>,
    recent_error: Option<String>,
    logs: VecDeque<DesktopLogEntryDto>,
}

const DESKTOP_NOTIFICATION_TITLE: &str = "uHorse Node Desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopNotificationKind {
    Test,
    Info,
    Warn,
    Error,
}

impl DesktopNotificationKind {
    fn as_log_message(self) -> &'static str {
        match self {
            Self::Test => "Test notification dispatched",
            Self::Info => "Info notification dispatched",
            Self::Warn => "Warning notification dispatched",
            Self::Error => "Error notification dispatched",
        }
    }

    fn as_protocol_kind(self) -> NotificationEventKind {
        match self {
            Self::Test => NotificationEventKind::Test,
            Self::Info => NotificationEventKind::Info,
            Self::Warn => NotificationEventKind::Warn,
            Self::Error => NotificationEventKind::Error,
        }
    }
}

#[derive(Debug, Clone)]
struct DesktopNotification {
    kind: DesktopNotificationKind,
    title: String,
    subtitle: Option<String>,
    message: String,
}

impl DesktopNotification {
    fn new(
        kind: DesktopNotificationKind,
        title: impl Into<String>,
        subtitle: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            subtitle: subtitle.map(|value| value.to_string()),
            message: message.into(),
        }
    }

    fn workspace_validation_failed(message: impl Into<String>) -> Self {
        Self::new(
            DesktopNotificationKind::Warn,
            DESKTOP_NOTIFICATION_TITLE,
            Some("工作区校验失败"),
            message,
        )
    }

    fn node_started() -> Self {
        Self::new(
            DesktopNotificationKind::Info,
            DESKTOP_NOTIFICATION_TITLE,
            Some("节点启动成功"),
            "Node started successfully",
        )
    }

    fn node_start_failed(message: impl Into<String>) -> Self {
        Self::new(
            DesktopNotificationKind::Error,
            DESKTOP_NOTIFICATION_TITLE,
            Some("节点启动失败"),
            message,
        )
    }

    fn node_stopped() -> Self {
        Self::new(
            DesktopNotificationKind::Info,
            DESKTOP_NOTIFICATION_TITLE,
            Some("节点停止成功"),
            "Node stopped",
        )
    }

    fn node_stop_failed(message: impl Into<String>) -> Self {
        Self::new(
            DesktopNotificationKind::Error,
            DESKTOP_NOTIFICATION_TITLE,
            Some("节点停止失败"),
            message,
        )
    }
}

impl DesktopAppState {
    pub fn new(config_store: ConfigStore) -> NodeResult<Self> {
        let config = config_store.load()?;
        Ok(Self {
            inner: Arc::new(RwLock::new(DesktopApp {
                config_store,
                config: config.node,
                desktop: config.desktop,
                lifecycle_state: DesktopLifecycleState::Stopped,
                node: None,
                recent_error: None,
                logs: VecDeque::new(),
            })),
        })
    }

    pub async fn get_settings(&self) -> DesktopSettingsDto {
        let app = self.inner.read().await;
        DesktopSettingsDto::from_config(&app.config, &app.desktop)
    }

    pub async fn default_settings(&self) -> DefaultSettingsDto {
        let desktop = DesktopPreferencesConfig::default();
        DefaultSettingsDto {
            suggested_name: current_computer_name(),
            notifications_enabled: desktop.notifications_enabled,
            show_notification_details: desktop.show_notification_details,
            mirror_notifications_to_dingtalk: desktop.mirror_notifications_to_dingtalk,
            launch_at_login: desktop.launch_at_login,
        }
    }

    pub async fn capability_status(&self) -> DesktopCapabilityStatusDto {
        let app = self.inner.read().await;
        let launch_agent_path = launch_agent_path()
            .ok()
            .map(|path| path.display().to_string());
        let launch_agent_installed = launch_agent_path
            .as_ref()
            .map(|path| Path::new(path).exists())
            .unwrap_or(false);

        DesktopCapabilityStatusDto {
            notifications_enabled: app.desktop.notifications_enabled,
            show_notification_details: app.desktop.show_notification_details,
            mirror_notifications_to_dingtalk: app.desktop.mirror_notifications_to_dingtalk,
            launch_at_login: app.desktop.launch_at_login,
            launch_agent_installed,
            launch_agent_path,
        }
    }

    pub async fn save_settings(
        &self,
        settings: DesktopSettingsDto,
    ) -> NodeResult<DesktopSettingsDto> {
        let mut app = self.inner.write().await;
        let previous_launch_at_login = app.desktop.launch_at_login;

        let mut next_config = app.config.clone();
        let mut next_desktop = app.desktop.clone();
        settings.apply_to_config(&mut next_config, &mut next_desktop);
        app.config = next_config;
        app.desktop = next_desktop;

        let desktop_config = DesktopConfig {
            node: app.config.clone(),
            desktop: app.desktop.clone(),
        };
        app.config_store.save(&desktop_config)?;

        sync_launch_agent(app.config_store.path(), app.desktop.launch_at_login)?;

        let workspace_path = app.config.workspace_path.clone();
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!("Settings saved for workspace {}", workspace_path),
            "settings.save",
        );

        let (_, restart_required, restart_notice) = runtime_config_status(&app);
        if restart_required {
            push_log(
                &mut app.logs,
                DesktopLogLevel::Info,
                restart_notice.clone().unwrap_or_else(|| {
                    "Settings saved and will take effect after restart".to_string()
                }),
                "settings.save",
            );
        }

        if previous_launch_at_login != app.desktop.launch_at_login {
            let launch_at_login_enabled = app.desktop.launch_at_login;
            push_log(
                &mut app.logs,
                DesktopLogLevel::Info,
                if launch_at_login_enabled {
                    "Launch at login enabled"
                } else {
                    "Launch at login disabled"
                },
                "settings.launch_at_login",
            );
        }

        Ok(DesktopSettingsDto::from_config(&app.config, &app.desktop))
    }

    pub async fn test_notification(&self) -> NodeResult<String> {
        let mut app = self.inner.write().await;
        let message = if app.desktop.show_notification_details {
            format!(
                "节点：{}\n工作区：{}",
                app.config.name, app.config.workspace_path
            )
        } else {
            "uHorse Node Desktop 当前可以正常弹出系统通知。".to_string()
        };

        let notification = DesktopNotification::new(
            DesktopNotificationKind::Test,
            DESKTOP_NOTIFICATION_TITLE,
            Some("测试通知"),
            message,
        );
        dispatch_notification(&mut app, notification, "settings.notifications")?;

        Ok("测试通知已发送，请检查系统通知中心。".to_string())
    }

    pub async fn pick_workspace(&self) -> NodeResult<String> {
        let mut app = self.inner.write().await;
        let path = pick_workspace_directory()?;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!("Workspace selected: {}", path),
            "workspace.pick",
        );
        Ok(path)
    }

    pub async fn validate_workspace(
        &self,
        workspace_path: String,
        require_git_repo: bool,
    ) -> WorkspaceValidationDto {
        let validation = match Workspace::new(&workspace_path) {
            Ok(workspace) => {
                let git_repo = workspace.is_git_repo();
                let valid = !require_git_repo || git_repo;
                let error = if valid {
                    None
                } else {
                    Some(
                        "Workspace must be a git repository when require_git_repo is enabled"
                            .to_string(),
                    )
                };

                WorkspaceValidationDto {
                    valid,
                    path: workspace_path,
                    normalized_path: Some(workspace.root().display().to_string()),
                    name: Some(workspace.name().to_string()),
                    git_repo,
                    require_git_repo,
                    error,
                }
            }
            Err(error) => WorkspaceValidationDto {
                valid: false,
                path: workspace_path,
                normalized_path: None,
                name: None,
                git_repo: false,
                require_git_repo,
                error: Some(error.to_string()),
            },
        };

        let mut app = self.inner.write().await;
        let level = if validation.valid {
            DesktopLogLevel::Info
        } else {
            DesktopLogLevel::Warn
        };
        let message = if validation.valid {
            format!("Workspace validation passed: {}", validation.path)
        } else {
            format!(
                "Workspace validation failed: {}",
                validation.error.clone().unwrap_or_default()
            )
        };
        push_log(&mut app.logs, level, message.clone(), "workspace.validate");

        if !validation.valid {
            try_dispatch_notification(
                &mut app,
                DesktopNotification::workspace_validation_failed(message),
                "workspace.validate",
            );
        }

        validation
    }

    pub async fn start_node(&self) -> NodeResult<DesktopNodeStatusDto> {
        let mut app = self.inner.write().await;
        if matches!(app.lifecycle_state, DesktopLifecycleState::Running) {
            push_log(
                &mut app.logs,
                DesktopLogLevel::Info,
                "Start requested while node already running",
                "runtime.start",
            );
            return Self::build_status(&app, None).await;
        }

        app.lifecycle_state = DesktopLifecycleState::Starting;
        app.recent_error = None;
        let workspace_path = app.config.workspace_path.clone();
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!("Starting node for workspace {}", workspace_path),
            "runtime.start",
        );

        let mut node = match Node::new(app.config.clone()) {
            Ok(node) => node,
            Err(error) => {
                let error_message = error.to_string();
                app.lifecycle_state = DesktopLifecycleState::Failed;
                app.recent_error = Some(error_message.clone());
                push_log(
                    &mut app.logs,
                    DesktopLogLevel::Error,
                    error_message.clone(),
                    "runtime.start",
                );
                try_dispatch_notification(
                    &mut app,
                    DesktopNotification::node_start_failed(error_message),
                    "runtime.start",
                );
                return Err(error);
            }
        };

        if let Err(error) = node.start().await {
            let error_message = error.to_string();
            app.lifecycle_state = DesktopLifecycleState::Failed;
            app.recent_error = Some(error_message.clone());
            push_log(
                &mut app.logs,
                DesktopLogLevel::Error,
                error_message.clone(),
                "runtime.start",
            );
            try_dispatch_notification(
                &mut app,
                DesktopNotification::node_start_failed(error_message),
                "runtime.start",
            );
            return Err(error);
        }

        app.node = Some(node);
        app.lifecycle_state = DesktopLifecycleState::Running;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            "Node started successfully",
            "runtime.start",
        );
        try_dispatch_notification(
            &mut app,
            DesktopNotification::node_started(),
            "runtime.start",
        );
        Self::build_status(&app, None).await
    }

    pub async fn stop_node(&self) -> NodeResult<DesktopNodeStatusDto> {
        let mut app = self.inner.write().await;
        app.lifecycle_state = DesktopLifecycleState::Stopping;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            "Stopping node",
            "runtime.stop",
        );

        let stop_result = if let Some(node) = app.node.as_mut() {
            node.stop().await
        } else {
            Ok(())
        };

        if let Err(error) = stop_result {
            let error_message = error.to_string();
            app.lifecycle_state = DesktopLifecycleState::Failed;
            app.recent_error = Some(error_message.clone());
            push_log(
                &mut app.logs,
                DesktopLogLevel::Error,
                error_message.clone(),
                "runtime.stop",
            );
            try_dispatch_notification(
                &mut app,
                DesktopNotification::node_stop_failed(error_message),
                "runtime.stop",
            );
            return Err(error);
        }

        app.node = None;
        app.lifecycle_state = DesktopLifecycleState::Stopped;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            "Node stopped",
            "runtime.stop",
        );
        try_dispatch_notification(
            &mut app,
            DesktopNotification::node_stopped(),
            "runtime.stop",
        );
        Self::build_status(&app, None).await
    }

    pub async fn runtime_status(&self) -> NodeResult<DesktopNodeStatusDto> {
        let account_status = self.account_status().await.ok();
        let app = self.inner.read().await;
        Self::build_status(&app, account_status.as_ref()).await
    }

    pub async fn connection_diagnostics(&self) -> NodeResult<DesktopConnectionDiagnosticsDto> {
        let account_status = self.account_status().await.ok();
        let runtime_status = self.runtime_status().await?;
        let workspace_status = self.workspace_status().await;
        let app = self.inner.read().await;

        Ok(Self::build_connection_diagnostics(
            &app,
            runtime_status,
            workspace_status,
            account_status.as_ref(),
        ))
    }

    pub async fn recover_connection(&self) -> NodeResult<DesktopConnectionRecoveryResultDto> {
        let workspace_status = self.workspace_status().await;
        if !workspace_status.valid {
            let status = self.connection_diagnostics().await?;
            return Ok(DesktopConnectionRecoveryResultDto {
                success: false,
                action: "validate_workspace".to_string(),
                message: workspace_status
                    .error
                    .clone()
                    .unwrap_or_else(|| "Workspace validation failed".to_string()),
                status,
            });
        }

        let (needs_stop, missing_node_id, missing_auth_token, invalid_hub_url) = {
            let app = self.inner.read().await;
            (
                matches!(
                    app.lifecycle_state,
                    DesktopLifecycleState::Running | DesktopLifecycleState::Starting
                ),
                app.config
                    .node_id
                    .as_ref()
                    .map(|value| value.to_string())
                    .filter(|value| !value.trim().is_empty())
                    .is_none(),
                app.config
                    .connection
                    .auth_token
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                    .is_none(),
                hub_http_base_url(&app.config.connection.hub_url).is_err(),
            )
        };

        let failure_message = if missing_node_id {
            Some("Node ID is missing".to_string())
        } else if missing_auth_token {
            Some("Node auth token is missing".to_string())
        } else if invalid_hub_url {
            Some("Hub URL is invalid".to_string())
        } else {
            None
        };

        if let Some(message) = failure_message {
            let status = self.connection_diagnostics().await?;
            return Ok(DesktopConnectionRecoveryResultDto {
                success: false,
                action: "check_prerequisites".to_string(),
                message,
                status,
            });
        }

        if needs_stop {
            self.stop_node().await?;
        }
        self.start_node().await?;
        let status = self.connection_diagnostics().await?;

        Ok(DesktopConnectionRecoveryResultDto {
            success: true,
            action: if needs_stop {
                "restart_node".to_string()
            } else {
                "start_node".to_string()
            },
            message: "Connection recovery executed".to_string(),
            status,
        })
    }

    pub async fn workspace_status(&self) -> DesktopWorkspaceStatusDto {
        let app = self.inner.read().await;
        let config = &app.config;

        let (running_workspace_path, restart_required, restart_notice) =
            runtime_config_status(&app);

        match Workspace::new(&config.workspace_path) {
            Ok(workspace) => DesktopWorkspaceStatusDto {
                valid: true,
                name: Some(workspace.name().to_string()),
                path: config.workspace_path.clone(),
                normalized_path: Some(workspace.root().display().to_string()),
                read_only: workspace.config().read_only,
                git_repo: workspace.is_git_repo(),
                require_git_repo: config.require_git_repo,
                watch_workspace: config.watch_workspace,
                git_protection_enabled: config.git_protection_enabled,
                auto_git_add_new_files: config.auto_git_add_new_files,
                internal_work_dir: config.internal_work_dir.clone(),
                allowed_patterns: workspace.config().allowed_patterns.clone(),
                denied_patterns: workspace.config().denied_patterns.clone(),
                running_workspace_path,
                restart_required,
                restart_notice,
                error: None,
            },
            Err(error) => DesktopWorkspaceStatusDto {
                valid: false,
                name: None,
                path: config.workspace_path.clone(),
                normalized_path: None,
                read_only: false,
                git_repo: false,
                require_git_repo: config.require_git_repo,
                watch_workspace: config.watch_workspace,
                git_protection_enabled: config.git_protection_enabled,
                auto_git_add_new_files: config.auto_git_add_new_files,
                internal_work_dir: config.internal_work_dir.clone(),
                allowed_patterns: vec![],
                denied_patterns: vec![],
                running_workspace_path,
                restart_required,
                restart_notice,
                error: Some(error.to_string()),
            },
        }
    }

    pub async fn version_summary(&self) -> DesktopVersionSummaryDto {
        let app = self.inner.read().await;
        let workspace = match Workspace::new(&app.config.workspace_path) {
            Ok(workspace) => Arc::new(workspace),
            Err(error) => {
                return DesktopVersionSummaryDto {
                    available: false,
                    error: Some(error.to_string()),
                    branch: None,
                    dirty: false,
                    entries: vec![],
                    current_checkpoint: None,
                    checkpoints: vec![],
                };
            }
        };

        let manager = VersionManager::new(workspace);
        match manager.status() {
            Ok(status) => {
                let current_checkpoint = manager
                    .current_checkpoint()
                    .ok()
                    .map(DesktopCheckpointDto::from_record);
                let checkpoints = manager
                    .list_checkpoints(10)
                    .unwrap_or_default()
                    .into_iter()
                    .map(DesktopCheckpointDto::from_record)
                    .collect();

                DesktopVersionSummaryDto {
                    available: true,
                    error: None,
                    branch: Some(status.branch),
                    dirty: status.dirty,
                    entries: status
                        .entries
                        .into_iter()
                        .map(|entry| DesktopVersionEntryDto {
                            path: entry.path,
                            staged_status: format!("{:?}", entry.staged_status).to_lowercase(),
                            unstaged_status: format!("{:?}", entry.unstaged_status).to_lowercase(),
                        })
                        .collect(),
                    current_checkpoint,
                    checkpoints,
                }
            }
            Err(error) => DesktopVersionSummaryDto {
                available: false,
                error: Some(error.to_string()),
                branch: None,
                dirty: false,
                entries: vec![],
                current_checkpoint: None,
                checkpoints: vec![],
            },
        }
    }

    pub async fn logs(&self) -> Vec<DesktopLogEntryDto> {
        let app = self.inner.read().await;
        app.logs.iter().cloned().collect()
    }

    pub async fn start_account_pairing(&self) -> NodeResult<DesktopPairingRequestDto> {
        let (hub_url, node_id, node_name, auth_token) = {
            let app = self.inner.read().await;
            let node_id = app
                .config
                .node_id
                .as_ref()
                .map(ToString::to_string)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| NodeError::Config("Node ID is missing".to_string()))?;
            let auth_token = app
                .config
                .connection
                .auth_token
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| NodeError::Permission("Node auth token is missing".to_string()))?;
            (
                app.config.connection.hub_url.clone(),
                node_id,
                app.config.name.clone(),
                auth_token,
            )
        };

        let request = StartAccountPairingRequest {
            node_id,
            node_name,
            device_type: DEFAULT_DEVICE_TYPE.to_string(),
        };
        let response = post_hub_api::<_, DesktopPairingRequestDto>(
            &hub_url,
            "/api/account/pairing/start",
            &request,
            Some(&auth_token),
        )
        .await?;

        let mut app = self.inner.write().await;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!(
                "Account pairing started for node {} with code {}",
                response.node_id, response.pairing_code
            ),
            "account.pairing.start",
        );
        Ok(response)
    }

    pub async fn cancel_account_pairing(&self, request_id: String) -> NodeResult<String> {
        let (hub_url, auth_token) = {
            let app = self.inner.read().await;
            let auth_token = app
                .config
                .connection
                .auth_token
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| NodeError::Permission("Node auth token is missing".to_string()))?;
            (app.config.connection.hub_url.clone(), auth_token)
        };

        let request = CancelAccountPairingRequest {
            request_id: request_id.clone(),
        };
        let response = post_hub_api::<_, String>(
            &hub_url,
            "/api/account/pairing/cancel",
            &request,
            Some(&auth_token),
        )
        .await?;

        let mut app = self.inner.write().await;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!("Account pairing cancelled: {}", request_id),
            "account.pairing.cancel",
        );
        Ok(response)
    }

    pub async fn account_status(&self) -> NodeResult<DesktopAccountStatusDto> {
        let (hub_url, node_id, auth_token) = account_api_credentials(&self.inner).await?;

        get_hub_api(
            &hub_url,
            &format!("/api/account/status/{}", node_id),
            Some(&auth_token),
        )
        .await
    }

    pub async fn delete_account_binding(&self) -> NodeResult<String> {
        let (hub_url, node_id, auth_token) = account_api_credentials(&self.inner).await?;

        let response = delete_hub_api::<String>(
            &hub_url,
            &format!("/api/account/binding/{}", node_id),
            Some(&auth_token),
        )
        .await?;

        let mut app = self.inner.write().await;
        push_log(
            &mut app.logs,
            DesktopLogLevel::Info,
            format!("Account binding removed for node {}", node_id),
            "account.binding.delete",
        );
        Ok(response)
    }

    fn build_connection_diagnostics(
        app: &DesktopApp,
        runtime_status: DesktopNodeStatusDto,
        workspace_status: DesktopWorkspaceStatusDto,
        account_status: Option<&DesktopAccountStatusDto>,
    ) -> DesktopConnectionDiagnosticsDto {
        let bound_user_id = account_status
            .and_then(|status| status.bound_user_id.clone())
            .or(runtime_status.bound_user_id.clone());

        DesktopConnectionDiagnosticsDto {
            lifecycle_state: runtime_status.lifecycle_state,
            connection_state: runtime_status.connection_state,
            overview_state: runtime_status.overview_state,
            overview_message: runtime_status.overview_message,
            hub_url: runtime_status.hub_url,
            node_id: runtime_status.node_id,
            bound_user_id,
            auth_token_present: app
                .config
                .connection
                .auth_token
                .as_ref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
            workspace_valid: workspace_status.valid,
            workspace_error: workspace_status.error,
            restart_required: runtime_status.restart_required,
            restart_notice: runtime_status.restart_notice,
            reconnect_interval_secs: app.config.connection.reconnect_interval_secs,
            max_reconnect_attempts: app.config.connection.max_reconnect_attempts,
            recent_error: runtime_status.recent_error,
            recent_logs: app.logs.iter().take(5).cloned().collect(),
        }
    }

    async fn build_status(
        app: &DesktopApp,
        account_status: Option<&DesktopAccountStatusDto>,
    ) -> NodeResult<DesktopNodeStatusDto> {
        let (runtime_workspace_path, restart_required, restart_notice) = runtime_config_status(app);
        let (pairing_enabled, bound_user_id, active_pairing_code, active_pairing_status) =
            account_overview(account_status);

        if let Some(node) = app.node.as_ref() {
            let metrics = node.get_metrics().await;
            let heartbeat = node
                .heartbeat_snapshot()
                .await
                .map(DesktopHeartbeatDto::from_snapshot);
            let connection_state = connection_state_label(&node.connection_state().await);
            let workspace = node.workspace();
            let latest_checkpoint = if workspace.is_git_repo() {
                VersionManager::new(workspace)
                    .current_checkpoint()
                    .ok()
                    .map(DesktopCheckpointDto::from_record)
            } else {
                None
            };
            let (overview_state, overview_message) = describe_runtime_overview(
                &app.lifecycle_state,
                &connection_state,
                restart_required,
                restart_notice.as_deref(),
                pairing_enabled,
                bound_user_id.as_deref(),
                active_pairing_status.as_deref(),
                app.recent_error.as_deref(),
            );

            return Ok(DesktopNodeStatusDto {
                name: node.config().name.clone(),
                node_id: Some(node.node_id().to_string()),
                lifecycle_state: app.lifecycle_state.clone(),
                connection_state,
                overview_state,
                overview_message,
                pairing_enabled,
                bound_user_id,
                active_pairing_code,
                active_pairing_status,
                hub_url: node.config().connection.hub_url.clone(),
                workspace_path: runtime_workspace_path
                    .clone()
                    .unwrap_or_else(|| node.config().workspace_path.clone()),
                saved_workspace_path: app.config.workspace_path.clone(),
                runtime_workspace_path,
                restart_required,
                restart_notice,
                running_tasks: node.running_tasks_count().await,
                max_concurrent_tasks: node.config().max_concurrent_tasks,
                pending_approvals: node.pending_approvals_count().await,
                metrics: DesktopMetricsDto::from_metrics(&metrics),
                heartbeat,
                latest_checkpoint,
                recent_error: app.recent_error.clone(),
            });
        }

        let workspace = Workspace::new(&app.config.workspace_path)
            .ok()
            .map(Arc::new);
        let latest_checkpoint = workspace
            .filter(|workspace| workspace.is_git_repo())
            .and_then(|workspace| VersionManager::new(workspace).current_checkpoint().ok())
            .map(DesktopCheckpointDto::from_record);
        let connection_state = "disconnected".to_string();
        let (overview_state, overview_message) = describe_runtime_overview(
            &app.lifecycle_state,
            &connection_state,
            restart_required,
            restart_notice.as_deref(),
            pairing_enabled,
            bound_user_id.as_deref(),
            active_pairing_status.as_deref(),
            app.recent_error.as_deref(),
        );

        Ok(DesktopNodeStatusDto {
            name: app.config.name.clone(),
            node_id: app.config.node_id.as_ref().map(ToString::to_string),
            lifecycle_state: app.lifecycle_state.clone(),
            connection_state,
            overview_state,
            overview_message,
            pairing_enabled,
            bound_user_id,
            active_pairing_code,
            active_pairing_status,
            hub_url: app.config.connection.hub_url.clone(),
            workspace_path: app.config.workspace_path.clone(),
            saved_workspace_path: app.config.workspace_path.clone(),
            runtime_workspace_path,
            restart_required,
            restart_notice,
            running_tasks: 0,
            max_concurrent_tasks: app.config.max_concurrent_tasks,
            pending_approvals: 0,
            metrics: DesktopMetricsDto::from_metrics(&Default::default()),
            heartbeat: None,
            latest_checkpoint,
            recent_error: app.recent_error.clone(),
        })
    }
}

async fn account_api_credentials(
    inner: &Arc<RwLock<DesktopApp>>,
) -> NodeResult<(String, String, String)> {
    let app = inner.read().await;
    let node_id = app
        .config
        .node_id
        .as_ref()
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| NodeError::Config("Node ID is missing".to_string()))?;
    let auth_token = app
        .config
        .connection
        .auth_token
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| NodeError::Permission("Node auth token is missing".to_string()))?;

    Ok((app.config.connection.hub_url.clone(), node_id, auth_token))
}

fn apply_hub_api_auth(
    request: reqwest::RequestBuilder,
    auth_token: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(token) = auth_token.filter(|value| !value.trim().is_empty()) {
        request.header(
            header::AUTHORIZATION,
            format!("{}{}", HUB_API_AUTH_HEADER_PREFIX, token),
        )
    } else {
        request
    }
}

fn hub_http_base_url(hub_url: &str) -> NodeResult<String> {
    if let Some(rest) = hub_url.strip_prefix("ws://") {
        let host = rest.strip_suffix("/ws").unwrap_or(rest);
        return Ok(format!("http://{}", host.trim_end_matches('/')));
    }
    if let Some(rest) = hub_url.strip_prefix("wss://") {
        let host = rest.strip_suffix("/ws").unwrap_or(rest);
        return Ok(format!("https://{}", host.trim_end_matches('/')));
    }
    if hub_url.starts_with("http://") || hub_url.starts_with("https://") {
        return Ok(hub_url.trim_end_matches('/').to_string());
    }
    Err(NodeError::Config(format!(
        "Unsupported Hub URL: {}",
        hub_url
    )))
}

async fn parse_hub_response<T: DeserializeOwned>(response: reqwest::Response) -> NodeResult<T> {
    let status = response.status();
    let text = response.text().await.map_err(|error| {
        NodeError::Connection(format!("Failed to read Hub response: {}", error))
    })?;
    let payload: ApiResponse<T> = serde_json::from_str(&text).map_err(NodeError::Serialization)?;

    if status.is_success() {
        payload
            .data
            .ok_or_else(|| NodeError::Internal("Hub response missing data field".to_string()))
    } else {
        let message = payload
            .error
            .map(|error| error.message)
            .unwrap_or_else(|| format!("Hub request failed with status {}", status));
        Err(match status {
            StatusCode::BAD_REQUEST => NodeError::Config(message),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => NodeError::Permission(message),
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE => {
                NodeError::Connection(message)
            }
            _ if status.is_server_error() => NodeError::Internal(message),
            _ => NodeError::Connection(message),
        })
    }
}

async fn get_hub_api<T: DeserializeOwned>(
    hub_url: &str,
    path: &str,
    auth_token: Option<&str>,
) -> NodeResult<T> {
    let base_url = hub_http_base_url(hub_url)?;
    let response = apply_hub_api_auth(
        reqwest::Client::new().get(format!("{}{}", base_url, path)),
        auth_token,
    )
    .send()
    .await
    .map_err(|error| NodeError::Connection(format!("Failed to call Hub API: {}", error)))?;
    parse_hub_response(response).await
}

async fn post_hub_api<P: serde::Serialize, T: DeserializeOwned>(
    hub_url: &str,
    path: &str,
    payload: &P,
    auth_token: Option<&str>,
) -> NodeResult<T> {
    let base_url = hub_http_base_url(hub_url)?;
    let response = apply_hub_api_auth(
        reqwest::Client::new()
            .post(format!("{}{}", base_url, path))
            .json(payload),
        auth_token,
    )
    .send()
    .await
    .map_err(|error| NodeError::Connection(format!("Failed to call Hub API: {}", error)))?;
    parse_hub_response(response).await
}

async fn delete_hub_api<T: DeserializeOwned>(
    hub_url: &str,
    path: &str,
    auth_token: Option<&str>,
) -> NodeResult<T> {
    let base_url = hub_http_base_url(hub_url)?;
    let response = apply_hub_api_auth(
        reqwest::Client::new().delete(format!("{}{}", base_url, path)),
        auth_token,
    )
    .send()
    .await
    .map_err(|error| NodeError::Connection(format!("Failed to call Hub API: {}", error)))?;
    parse_hub_response(response).await
}

fn runtime_config_status(app: &DesktopApp) -> (Option<String>, bool, Option<String>) {
    let runtime_config = app.node.as_ref().map(|node| node.config());
    let runtime_workspace_path = runtime_config.map(|config| config.workspace_path.clone());
    let restart_required = runtime_config
        .map(|config| node_config_requires_restart(config, &app.config))
        .unwrap_or(false);
    let restart_notice = if restart_required {
        if runtime_workspace_path.as_deref() != Some(app.config.workspace_path.as_str()) {
            Some(format!(
                "设置已保存，需重启 Node 后工作区才会从 {} 切换到 {}",
                runtime_workspace_path.as_deref().unwrap_or("-"),
                app.config.workspace_path
            ))
        } else {
            Some("设置已保存，需重启 Node 后运行中的配置才会生效".to_string())
        }
    } else {
        None
    };

    (runtime_workspace_path, restart_required, restart_notice)
}

fn account_overview(
    account_status: Option<&DesktopAccountStatusDto>,
) -> (bool, Option<String>, Option<String>, Option<String>) {
    let Some(status) = account_status else {
        return (false, None, None, None);
    };

    (
        status.pairing_enabled,
        status.bound_user_id.clone(),
        status
            .pairing
            .as_ref()
            .map(|pairing| pairing.pairing_code.clone()),
        status
            .pairing
            .as_ref()
            .map(|pairing| pairing.status.clone()),
    )
}

fn describe_runtime_overview(
    lifecycle_state: &DesktopLifecycleState,
    connection_state: &str,
    restart_required: bool,
    restart_notice: Option<&str>,
    pairing_enabled: bool,
    bound_user_id: Option<&str>,
    active_pairing_status: Option<&str>,
    recent_error: Option<&str>,
) -> (String, String) {
    if let Some(error) = recent_error.filter(|value| !value.trim().is_empty()) {
        if matches!(lifecycle_state, DesktopLifecycleState::Failed)
            || connection_state.starts_with("failed")
        {
            return ("error".to_string(), error.to_string());
        }
    }

    match lifecycle_state {
        DesktopLifecycleState::Starting => {
            return ("starting".to_string(), "Node 正在启动。".to_string())
        }
        DesktopLifecycleState::Stopping => {
            return ("stopping".to_string(), "Node 正在停止。".to_string())
        }
        DesktopLifecycleState::Failed => {
            return (
                "error".to_string(),
                recent_error
                    .unwrap_or("Node 启动失败，请检查最近错误。")
                    .to_string(),
            )
        }
        DesktopLifecycleState::Stopped => {
            if restart_required {
                return (
                    "attention".to_string(),
                    restart_notice
                        .unwrap_or("设置已保存，需重启 Node 后生效。")
                        .to_string(),
                );
            }

            return ("idle".to_string(), "Node 尚未启动。".to_string());
        }
        DesktopLifecycleState::Running => {}
    }

    if restart_required {
        return (
            "attention".to_string(),
            restart_notice
                .unwrap_or("设置已保存，需重启 Node 后生效。")
                .to_string(),
        );
    }

    if bound_user_id.is_some() {
        return (
            "bound".to_string(),
            "Node 运行中，DingTalk 账号已绑定。".to_string(),
        );
    }

    if let Some(status) = active_pairing_status {
        if matches!(status, "pending" | "awaiting_confirmation") {
            return (
                "pairing".to_string(),
                "绑定码已生成，等待在 DingTalk 中确认。".to_string(),
            );
        }
    }

    if pairing_enabled {
        return (
            "unbound".to_string(),
            "Node 运行中，尚未绑定 DingTalk 账号。".to_string(),
        );
    }

    if connection_state.starts_with("authenticated") {
        return (
            "running".to_string(),
            "Node 运行中，Hub 连接正常。".to_string(),
        );
    }

    if connection_state.starts_with("authenticating") {
        return (
            "connecting".to_string(),
            "Node 运行中，正在等待 Hub 确认注册。".to_string(),
        );
    }

    if connection_state.starts_with("connected") {
        return (
            "connecting".to_string(),
            "Node 运行中，已连接 Hub，等待注册确认。".to_string(),
        );
    }

    if connection_state.starts_with("connecting") || connection_state.starts_with("reconnecting") {
        return (
            "connecting".to_string(),
            "Node 运行中，正在连接 Hub。".to_string(),
        );
    }

    ("running".to_string(), "Node 运行中。".to_string())
}

fn node_config_requires_restart(runtime_config: &NodeConfig, saved_config: &NodeConfig) -> bool {
    runtime_config.name != saved_config.name
        || runtime_config.connection.hub_url != saved_config.connection.hub_url
        || runtime_config.workspace_path != saved_config.workspace_path
        || runtime_config.require_git_repo != saved_config.require_git_repo
        || runtime_config.watch_workspace != saved_config.watch_workspace
        || runtime_config.git_protection_enabled != saved_config.git_protection_enabled
        || runtime_config.auto_git_add_new_files != saved_config.auto_git_add_new_files
}

fn connection_state_label(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Disconnected => "disconnected".to_string(),
        ConnectionState::Connecting => "connecting".to_string(),
        ConnectionState::Connected { .. } => "connected".to_string(),
        ConnectionState::Authenticating => "authenticating".to_string(),
        ConnectionState::Authenticated { .. } => "authenticated".to_string(),
        ConnectionState::Reconnecting { attempt } => format!("reconnecting ({})", attempt),
        ConnectionState::Failed { error } => format!("failed: {}", error),
    }
}

impl DesktopMetricsDto {
    fn from_metrics(metrics: &uhorse_node_runtime::Metrics) -> Self {
        Self {
            total_executions: metrics.total_executions,
            successful_executions: metrics.successful_executions,
            failed_executions: metrics.failed_executions,
            avg_duration_ms: metrics.avg_duration_ms,
            success_rate: metrics.success_rate(),
        }
    }
}

impl DesktopHeartbeatDto {
    fn from_snapshot(snapshot: uhorse_node_runtime::status::HeartbeatSnapshot) -> Self {
        Self {
            cpu_percent: snapshot.status.cpu_percent,
            memory_mb: snapshot.status.memory_mb,
            disk_gb: snapshot.status.disk_gb,
            network_latency_ms: snapshot.status.network_latency_ms,
            last_heartbeat: snapshot.status.last_heartbeat.to_rfc3339(),
        }
    }
}

impl DesktopCheckpointDto {
    fn from_record(record: uhorse_node_runtime::CheckpointRecord) -> Self {
        Self {
            revision: record.revision,
            message: record.message,
            created_at: record.created_at.to_rfc3339(),
        }
    }
}

fn push_log(
    logs: &mut VecDeque<DesktopLogEntryDto>,
    level: DesktopLogLevel,
    message: impl Into<String>,
    source: impl Into<String>,
) {
    logs.push_front(DesktopLogEntryDto {
        level,
        message: message.into(),
        timestamp: Utc::now().to_rfc3339(),
        source: source.into(),
    });

    while logs.len() > 200 {
        logs.pop_back();
    }
}

fn try_dispatch_notification(
    app: &mut DesktopApp,
    notification: DesktopNotification,
    source: &str,
) {
    if let Err(error) = dispatch_notification(app, notification, source) {
        push_log(
            &mut app.logs,
            DesktopLogLevel::Warn,
            format!("Failed to dispatch notification: {}", error),
            source,
        );
    }
}

fn dispatch_notification(
    app: &mut DesktopApp,
    notification: DesktopNotification,
    source: &str,
) -> NodeResult<()> {
    if !app.desktop.notifications_enabled {
        return Err(NodeError::Config(
            "通知功能已关闭，请先开启通知功能".to_string(),
        ));
    }

    show_system_notification(
        &notification.title,
        notification.subtitle.as_deref(),
        &notification.message,
    )?;

    push_log(
        &mut app.logs,
        DesktopLogLevel::Info,
        notification.kind.as_log_message(),
        source,
    );

    if app.desktop.mirror_notifications_to_dingtalk {
        let mirror_result = if let Some(node) = app.node.as_ref() {
            node.report_notification_event_nowait(
                notification.kind.as_protocol_kind(),
                notification.title.clone(),
                notification.message.clone(),
                app.desktop.show_notification_details,
            )
        } else {
            Err(NodeError::Connection("Node is not running".to_string()))
        };

        if let Err(error) = mirror_result {
            push_log(
                &mut app.logs,
                DesktopLogLevel::Warn,
                format!("Failed to mirror notification to Hub: {}", error),
                source,
            );
        }
    }

    Ok(())
}

fn sync_launch_agent(config_path: &Path, enabled: bool) -> NodeResult<()> {
    let launch_agent_path = launch_agent_path().map_err(NodeError::Internal)?;

    if enabled {
        write_launch_agent(&launch_agent_path, config_path).map_err(NodeError::Internal)?;
        return Ok(());
    }

    match fs::remove_file(&launch_agent_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(NodeError::Internal(format!(
            "Failed to remove launch agent {}: {}",
            launch_agent_path.display(),
            error
        ))),
    }
}

fn write_launch_agent(launch_agent_path: &Path, config_path: &Path) -> Result<(), String> {
    let parent = launch_agent_path
        .parent()
        .ok_or_else(|| format!("Invalid launch agent path: {}", launch_agent_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create launch agent directory {}: {}",
            parent.display(),
            error
        )
    })?;

    let executable = std::env::current_exe()
        .map_err(|error| format!("Failed to resolve current executable: {}", error))?;
    let config_path = absolute_path(config_path).map_err(|error| {
        format!(
            "Failed to resolve config path {}: {}",
            config_path.display(),
            error
        )
    })?;
    let working_directory = std::env::current_dir()
        .map_err(|error| format!("Failed to resolve current working directory: {}", error))?;

    let plist = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n",
            "<dict>\n",
            "  <key>Label</key>\n",
            "  <string>{label}</string>\n",
            "  <key>ProgramArguments</key>\n",
            "  <array>\n",
            "    <string>{exe}</string>\n",
            "    <string>--config</string>\n",
            "    <string>{config}</string>\n",
            "    <string>serve</string>\n",
            "    <string>--listen</string>\n",
            "    <string>{listen}</string>\n",
            "  </array>\n",
            "  <key>RunAtLoad</key>\n",
            "  <true/>\n",
            "  <key>KeepAlive</key>\n",
            "  <false/>\n",
            "  <key>WorkingDirectory</key>\n",
            "  <string>{working_directory}</string>\n",
            "</dict>\n",
            "</plist>\n"
        ),
        label = xml_escape(DESKTOP_LAUNCH_AGENT_ID),
        exe = xml_escape(&executable.display().to_string()),
        config = xml_escape(&config_path.display().to_string()),
        listen = xml_escape(DEFAULT_DESKTOP_LISTEN),
        working_directory = xml_escape(&working_directory.display().to_string()),
    );

    fs::write(launch_agent_path, plist).map_err(|error| {
        format!(
            "Failed to write launch agent {}: {}",
            launch_agent_path.display(),
            error
        )
    })
}

fn launch_agent_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME environment variable is not set".to_string())?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", DESKTOP_LAUNCH_AGENT_ID)))
}

fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn apple_script_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn show_system_notification(title: &str, subtitle: Option<&str>, message: &str) -> NodeResult<()> {
    #[cfg(target_os = "macos")]
    {
        let mut script = format!(
            "display notification \"{}\" with title \"{}\"",
            apple_script_string(message),
            apple_script_string(title)
        );
        if let Some(subtitle) = subtitle {
            script.push_str(&format!(" subtitle \"{}\"", apple_script_string(subtitle)));
        }

        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| {
                NodeError::Internal(format!("Failed to launch notification: {}", error))
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NodeError::Internal(if stderr.is_empty() {
            "Failed to dispatch notification".to_string()
        } else {
            format!("Failed to dispatch notification: {}", stderr)
        }));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, subtitle, message);
        Err(NodeError::Internal(
            "当前平台暂不支持系统通知测试".to_string(),
        ))
    }
}

fn pick_workspace_directory() -> NodeResult<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"选择 uHorse 工作区\")",
            ])
            .output()
            .map_err(|error| {
                NodeError::Internal(format!("Failed to open workspace picker: {}", error))
            })?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                return Err(NodeError::Config("未选择任何工作区路径".to_string()));
            }
            return Ok(path);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err(NodeError::Config("已取消选择工作区".to_string()));
        }

        Err(NodeError::Internal(if stderr.is_empty() {
            "打开工作区选择器失败".to_string()
        } else {
            format!("打开工作区选择器失败: {}", stderr)
        }))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(NodeError::Internal(
            "当前平台暂不支持工作区选择器".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        routing::{delete, get, post},
        Json, Router,
    };
    use tempfile::TempDir;
    use uhorse_node_runtime::NodeId;

    #[tokio::test]
    async fn test_save_settings_persists_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("node-desktop.toml");
        let state = DesktopAppState::new(ConfigStore::new(&config_path)).unwrap();

        let saved = state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url: "ws://localhost:8765/ws".to_string(),
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: true,
                notifications_enabled: false,
                show_notification_details: false,
                mirror_notifications_to_dingtalk: true,
                launch_at_login: false,
            })
            .await
            .unwrap();

        assert_eq!(saved.name, "Desktop Node");
        assert!(!saved.notifications_enabled);
        assert!(saved.mirror_notifications_to_dingtalk);
        let loaded = ConfigStore::new(&config_path).load().unwrap();
        assert_eq!(loaded.node.name, "Desktop Node");
        assert!(!loaded.desktop.notifications_enabled);
        assert!(!loaded.desktop.show_notification_details);
        assert!(loaded.desktop.mirror_notifications_to_dingtalk);
    }

    #[tokio::test]
    async fn test_default_settings_uses_computer_name() {
        let temp = TempDir::new().unwrap();
        let state =
            DesktopAppState::new(ConfigStore::new(temp.path().join("node-desktop.toml"))).unwrap();

        let defaults = state.default_settings().await;
        assert!(!defaults.suggested_name.trim().is_empty());
        assert!(defaults.notifications_enabled);
        assert!(defaults.show_notification_details);
        assert!(!defaults.mirror_notifications_to_dingtalk);
        assert!(!defaults.launch_at_login);
    }

    #[tokio::test]
    async fn test_validate_workspace_reports_missing_directory() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.desktop.notifications_enabled = false;
        store.save(&config).unwrap();
        let state = DesktopAppState::new(store).unwrap();

        let validation = state
            .validate_workspace(temp.path().join("missing").display().to_string(), true)
            .await;

        assert!(!validation.valid);
        assert!(validation.error.is_some());

        let logs = state.logs().await;
        assert!(logs.iter().any(|entry| {
            entry.source == "workspace.validate"
                && matches!(entry.level, DesktopLogLevel::Warn)
                && entry.message.starts_with("Workspace validation failed:")
        }));
    }

    #[tokio::test]
    async fn test_runtime_status_uses_saved_config_when_stopped() {
        let temp = TempDir::new().unwrap();
        let workspace_path = temp.path().to_string_lossy().to_string();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = workspace_path.clone();
        config.node.require_git_repo = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        let status = state.runtime_status().await.unwrap();

        assert_eq!(status.workspace_path, workspace_path);
        assert_eq!(status.saved_workspace_path, workspace_path);
        assert_eq!(status.runtime_workspace_path, None);
        assert!(!status.restart_required);
        assert_eq!(status.restart_notice, None);
        assert!(matches!(
            status.lifecycle_state,
            DesktopLifecycleState::Stopped
        ));
    }

    #[tokio::test]
    async fn test_workspace_status_reports_config_flags() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = temp.path().to_string_lossy().to_string();
        config.node.require_git_repo = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        let status = state.workspace_status().await;

        assert!(status.valid);
        assert_eq!(status.path, temp.path().to_string_lossy());
        assert!(!status.require_git_repo);
        assert_eq!(status.running_workspace_path, None);
        assert!(!status.restart_required);
        assert_eq!(status.restart_notice, None);
    }

    #[tokio::test]
    async fn test_version_summary_unavailable_for_missing_workspace() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = temp.path().join("missing").display().to_string();
        config.node.require_git_repo = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        let summary = state.version_summary().await;

        assert!(!summary.available);
        assert!(summary.error.is_some());
    }

    #[tokio::test]
    async fn test_start_and_stop_node_transitions() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = temp.path().to_string_lossy().to_string();
        config.node.require_git_repo = false;
        config.node.connection.hub_url = "ws://127.0.0.1:65535/ws".to_string();
        config.desktop.notifications_enabled = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        let started = state.start_node().await.unwrap();
        assert!(matches!(
            started.lifecycle_state,
            DesktopLifecycleState::Running
        ));
        assert_eq!(started.saved_workspace_path, temp.path().to_string_lossy());
        assert_eq!(
            started.runtime_workspace_path.as_deref(),
            Some(temp.path().to_string_lossy().as_ref())
        );
        assert!(!started.restart_required);

        let stopped = state.stop_node().await.unwrap();
        assert!(matches!(
            stopped.lifecycle_state,
            DesktopLifecycleState::Stopped
        ));
        assert_eq!(stopped.runtime_workspace_path, None);
        assert!(!stopped.restart_required);

        let logs = state.logs().await;
        assert!(logs
            .iter()
            .any(|entry| entry.source == "runtime.start"
                && entry.message == "Node started successfully"));
        assert!(logs
            .iter()
            .any(|entry| entry.source == "runtime.stop" && entry.message == "Node stopped"));
    }

    #[test]
    fn test_notification_kind_maps_to_protocol_kind() {
        assert!(matches!(
            DesktopNotificationKind::Test.as_protocol_kind(),
            NotificationEventKind::Test
        ));
        assert!(matches!(
            DesktopNotificationKind::Info.as_protocol_kind(),
            NotificationEventKind::Info
        ));
        assert!(matches!(
            DesktopNotificationKind::Warn.as_protocol_kind(),
            NotificationEventKind::Warn
        ));
        assert!(matches!(
            DesktopNotificationKind::Error.as_protocol_kind(),
            NotificationEventKind::Error
        ));
    }

    #[test]
    fn test_workspace_validation_failed_notification_shape() {
        let notification = DesktopNotification::workspace_validation_failed(
            "Workspace validation failed: missing .git",
        );

        assert_eq!(notification.kind, DesktopNotificationKind::Warn);
        assert_eq!(notification.title, DESKTOP_NOTIFICATION_TITLE);
        assert_eq!(notification.subtitle.as_deref(), Some("工作区校验失败"));
        assert_eq!(
            notification.message,
            "Workspace validation failed: missing .git"
        );
    }

    #[test]
    fn test_node_started_notification_shape() {
        let notification = DesktopNotification::node_started();

        assert_eq!(notification.kind, DesktopNotificationKind::Info);
        assert_eq!(notification.title, DESKTOP_NOTIFICATION_TITLE);
        assert_eq!(notification.subtitle.as_deref(), Some("节点启动成功"));
        assert_eq!(notification.message, "Node started successfully");
    }

    #[test]
    fn test_node_start_failed_notification_shape() {
        let notification = DesktopNotification::node_start_failed("connection refused");

        assert_eq!(notification.kind, DesktopNotificationKind::Error);
        assert_eq!(notification.title, DESKTOP_NOTIFICATION_TITLE);
        assert_eq!(notification.subtitle.as_deref(), Some("节点启动失败"));
        assert_eq!(notification.message, "connection refused");
    }

    #[test]
    fn test_node_stopped_notification_shape() {
        let notification = DesktopNotification::node_stopped();

        assert_eq!(notification.kind, DesktopNotificationKind::Info);
        assert_eq!(notification.title, DESKTOP_NOTIFICATION_TITLE);
        assert_eq!(notification.subtitle.as_deref(), Some("节点停止成功"));
        assert_eq!(notification.message, "Node stopped");
    }

    #[test]
    fn test_node_stop_failed_notification_shape() {
        let notification = DesktopNotification::node_stop_failed("permission denied");

        assert_eq!(notification.kind, DesktopNotificationKind::Error);
        assert_eq!(notification.title, DESKTOP_NOTIFICATION_TITLE);
        assert_eq!(notification.subtitle.as_deref(), Some("节点停止失败"));
        assert_eq!(notification.message, "permission denied");
    }

    #[tokio::test]
    async fn test_save_settings_marks_restart_required_when_runtime_workspace_changes() {
        let temp = TempDir::new().unwrap();
        let original_workspace = temp.path().join("workspace-a");
        let next_workspace = temp.path().join("workspace-b");
        std::fs::create_dir_all(&original_workspace).unwrap();
        std::fs::create_dir_all(&next_workspace).unwrap();

        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = original_workspace.display().to_string();
        config.node.require_git_repo = false;
        config.node.connection.hub_url = "ws://127.0.0.1:65535/ws".to_string();
        config.desktop.notifications_enabled = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        state.start_node().await.unwrap();

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: next_workspace.display().to_string(),
                hub_url: "ws://127.0.0.1:65535/ws".to_string(),
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: true,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let runtime = state.runtime_status().await.unwrap();
        assert_eq!(
            runtime.saved_workspace_path,
            next_workspace.display().to_string()
        );
        assert_eq!(
            runtime.workspace_path,
            original_workspace.display().to_string()
        );
        assert_eq!(
            runtime.runtime_workspace_path.as_deref(),
            Some(original_workspace.to_string_lossy().as_ref())
        );
        assert!(runtime.restart_required);
        assert_eq!(
            runtime.restart_notice,
            Some(format!(
                "设置已保存，需重启 Node 后工作区才会从 {} 切换到 {}",
                original_workspace.display(),
                next_workspace.display()
            ))
        );

        let workspace = state.workspace_status().await;
        assert_eq!(workspace.path, next_workspace.display().to_string());
        assert_eq!(
            workspace.running_workspace_path.as_deref(),
            Some(original_workspace.to_string_lossy().as_ref())
        );
        assert!(workspace.restart_required);
        assert_eq!(workspace.restart_notice, runtime.restart_notice);

        let logs = state.logs().await;
        assert!(logs.iter().any(|entry| {
            entry.source == "settings.save"
                && entry.message
                    == format!(
                        "设置已保存，需重启 Node 后工作区才会从 {} 切换到 {}",
                        original_workspace.display(),
                        next_workspace.display()
                    )
        }));
    }

    #[tokio::test]
    async fn test_restart_applies_saved_workspace_after_stop_and_start() {
        let temp = TempDir::new().unwrap();
        let original_workspace = temp.path().join("workspace-a");
        let next_workspace = temp.path().join("workspace-b");
        std::fs::create_dir_all(&original_workspace).unwrap();
        std::fs::create_dir_all(&next_workspace).unwrap();

        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.workspace_path = original_workspace.display().to_string();
        config.node.require_git_repo = false;
        config.node.connection.hub_url = "ws://127.0.0.1:65535/ws".to_string();
        config.desktop.notifications_enabled = false;
        store.save(&config).unwrap();

        let state = DesktopAppState::new(store).unwrap();
        state.start_node().await.unwrap();
        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: next_workspace.display().to_string(),
                hub_url: "ws://127.0.0.1:65535/ws".to_string(),
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: true,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        state.stop_node().await.unwrap();
        let restarted = state.start_node().await.unwrap();

        assert_eq!(
            restarted.saved_workspace_path,
            next_workspace.display().to_string()
        );
        assert_eq!(
            restarted.workspace_path,
            next_workspace.display().to_string()
        );
        assert_eq!(
            restarted.runtime_workspace_path.as_deref(),
            Some(next_workspace.to_string_lossy().as_ref())
        );
        assert!(!restarted.restart_required);
        assert_eq!(restarted.restart_notice, None);
    }

    fn create_account_test_state(temp: &TempDir) -> DesktopAppState {
        let config_path = temp.path().join("node-desktop.toml");
        let store = ConfigStore::new(&config_path);
        let mut config = DesktopConfig::default();
        config.node.node_id = Some(NodeId::from_string("node-desktop-test"));
        config.node.workspace_path = temp.path().to_string_lossy().to_string();
        config.node.require_git_repo = false;
        config.node.connection.hub_url = "http://127.0.0.1:18080".to_string();
        config.node.connection.auth_token = Some("desktop-token".to_string());
        config.desktop.notifications_enabled = false;
        store.save(&config).unwrap();
        DesktopAppState::new(store).unwrap()
    }

    fn create_mock_hub_router() -> Router {
        Router::new()
            .route(
                "/api/account/pairing/start",
                post(|headers: axum::http::HeaderMap| async move {
                    if headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        != Some("Bearer desktop-token")
                    {
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    Ok(Json(ApiResponse::success(DesktopPairingRequestDto {
                        request_id: "req-1".to_string(),
                        node_id: "node-desktop-test".to_string(),
                        node_name: "Desktop Node".to_string(),
                        device_type: "desktop".to_string(),
                        pairing_code: "123456".to_string(),
                        status: "pending".to_string(),
                        created_at: 1,
                        expires_at: 2,
                        bound_user_id: None,
                    })))
                }),
            )
            .route(
                "/api/account/pairing/cancel",
                post(|headers: axum::http::HeaderMap| async move {
                    if headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        != Some("Bearer desktop-token")
                    {
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    Ok(Json(ApiResponse::success("Pairing cancelled".to_string())))
                }),
            )
            .route(
                "/api/account/status/node-desktop-test",
                get(|headers: axum::http::HeaderMap| async move {
                    if headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        != Some("Bearer desktop-token")
                    {
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    Ok(Json(ApiResponse::success(DesktopAccountStatusDto {
                        node_id: "node-desktop-test".to_string(),
                        pairing_enabled: true,
                        bound_user_id: Some("ding-user-1".to_string()),
                        pairing: None,
                    })))
                }),
            )
            .route(
                "/api/account/binding/node-desktop-test",
                delete(|headers: axum::http::HeaderMap| async move {
                    if headers
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        != Some("Bearer desktop-token")
                    {
                        return Err(axum::http::StatusCode::UNAUTHORIZED);
                    }

                    Ok(Json(ApiResponse::success("Binding removed".to_string())))
                }),
            )
    }

    async fn spawn_mock_hub_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, create_mock_hub_router())
                .await
                .unwrap();
        });
        format!("http://{}", address)
    }

    #[tokio::test]
    async fn test_runtime_status_includes_account_overview_when_bound() {
        let temp = TempDir::new().unwrap();
        let state = create_account_test_state(&temp);
        let hub_url = spawn_mock_hub_server().await;

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url,
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let status = state.runtime_status().await.unwrap();
        assert!(status.pairing_enabled);
        assert_eq!(status.bound_user_id.as_deref(), Some("ding-user-1"));
        assert_eq!(status.active_pairing_code, None);
        assert_eq!(status.overview_state, "idle");
        assert_eq!(status.overview_message, "Node 尚未启动。");
    }

    #[test]
    fn test_describe_runtime_overview_requires_authenticated_for_healthy_hub_message() {
        let (state, message) = describe_runtime_overview(
            &DesktopLifecycleState::Running,
            "connected",
            false,
            None,
            false,
            None,
            None,
            None,
        );
        assert_eq!(state, "connecting");
        assert_eq!(message, "Node 运行中，已连接 Hub，等待注册确认。");

        let (state, message) = describe_runtime_overview(
            &DesktopLifecycleState::Running,
            "authenticating",
            false,
            None,
            false,
            None,
            None,
            None,
        );
        assert_eq!(state, "connecting");
        assert_eq!(message, "Node 运行中，正在等待 Hub 确认注册。");

        let (state, message) = describe_runtime_overview(
            &DesktopLifecycleState::Running,
            "authenticated",
            false,
            None,
            false,
            None,
            None,
            None,
        );
        assert_eq!(state, "running");
        assert_eq!(message, "Node 运行中，Hub 连接正常。");
    }

    #[tokio::test]
    async fn test_connection_diagnostics_reports_recent_logs_and_prerequisites() {
        let temp = TempDir::new().unwrap();
        let state = create_account_test_state(&temp);
        let hub_url = spawn_mock_hub_server().await;

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url,
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let _ = state
            .validate_workspace(temp.path().to_string_lossy().to_string(), false)
            .await;
        let diagnostics = state.connection_diagnostics().await.unwrap();

        assert_eq!(diagnostics.lifecycle_state, DesktopLifecycleState::Stopped);
        assert_eq!(diagnostics.connection_state, "disconnected");
        assert!(diagnostics.auth_token_present);
        assert!(diagnostics.workspace_valid);
        assert_eq!(diagnostics.bound_user_id.as_deref(), Some("ding-user-1"));
        assert!(!diagnostics.recent_logs.is_empty());
    }

    #[tokio::test]
    async fn test_recover_connection_returns_failure_for_invalid_workspace() {
        let temp = TempDir::new().unwrap();
        let state = create_account_test_state(&temp);

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().join("missing").to_string_lossy().to_string(),
                hub_url: "http://127.0.0.1:18080".to_string(),
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let result = state.recover_connection().await.unwrap();
        assert!(!result.success);
        assert_eq!(result.action, "validate_workspace");
        assert!(!result.status.workspace_valid);
    }

    #[tokio::test]
    async fn test_recover_connection_starts_node_when_prerequisites_are_ready() {
        let temp = TempDir::new().unwrap();
        let state = create_account_test_state(&temp);
        let hub_url = spawn_mock_hub_server().await;

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url,
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let result = state.recover_connection().await.unwrap();
        assert!(result.success);
        assert_eq!(result.action, "start_node");
        assert_eq!(
            result.status.lifecycle_state,
            DesktopLifecycleState::Running
        );
    }

    #[tokio::test]
    async fn test_account_api_methods_proxy_to_hub() {
        let temp = TempDir::new().unwrap();
        let state = create_account_test_state(&temp);
        let hub_url = spawn_mock_hub_server().await;

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url,
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: false,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let pairing = state.start_account_pairing().await.unwrap();
        assert_eq!(pairing.node_id, "node-desktop-test");
        assert_eq!(pairing.pairing_code, "123456");

        let status = state.account_status().await.unwrap();
        assert_eq!(status.node_id, "node-desktop-test");
        assert_eq!(status.bound_user_id.as_deref(), Some("ding-user-1"));
        assert!(status.pairing.is_none());

        let cancel = state
            .cancel_account_pairing("req-1".to_string())
            .await
            .unwrap();
        assert_eq!(cancel, "Pairing cancelled");

        let deleted = state.delete_account_binding().await.unwrap();
        assert_eq!(deleted, "Binding removed");

        let logs = state.logs().await;
        assert!(logs
            .iter()
            .any(|entry| entry.source == "account.pairing.start"));
        assert!(logs
            .iter()
            .any(|entry| entry.source == "account.pairing.cancel"));
        assert!(logs
            .iter()
            .any(|entry| entry.source == "account.binding.delete"));
    }

    #[tokio::test]
    async fn test_logs_capture_actions() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let state = DesktopAppState::new(store).unwrap();

        state
            .save_settings(DesktopSettingsDto {
                name: "Desktop Node".to_string(),
                workspace_path: temp.path().to_string_lossy().to_string(),
                hub_url: "ws://localhost:8765/ws".to_string(),
                require_git_repo: false,
                watch_workspace: true,
                git_protection_enabled: true,
                auto_git_add_new_files: false,
                notifications_enabled: true,
                show_notification_details: true,
                mirror_notifications_to_dingtalk: false,
                launch_at_login: false,
            })
            .await
            .unwrap();

        let logs = state.logs().await;
        assert!(!logs.is_empty());
        assert_eq!(logs[0].source, "settings.save");
    }
}
