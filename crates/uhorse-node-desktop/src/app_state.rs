use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use chrono::Utc;
use tokio::sync::RwLock;
use uhorse_node_runtime::{
    ConnectionState, Node, NodeConfig, NodeError, NodeResult, NotificationEventKind,
    VersionManager, Workspace,
};

use crate::config_store::{
    current_computer_name, ConfigStore, DesktopConfig, DesktopPreferencesConfig,
};
use crate::dto::{
    DefaultSettingsDto, DesktopCapabilityStatusDto, DesktopCheckpointDto, DesktopHeartbeatDto,
    DesktopLifecycleState, DesktopLogEntryDto, DesktopLogLevel, DesktopMetricsDto,
    DesktopNodeStatusDto, DesktopSettingsDto, DesktopVersionEntryDto, DesktopVersionSummaryDto,
    DesktopWorkspaceStatusDto, WorkspaceValidationDto,
};

const DESKTOP_LAUNCH_AGENT_ID: &str = "com.uhorse.node-desktop";
const DEFAULT_DESKTOP_LISTEN: &str = "127.0.0.1:8757";

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
            return Self::build_status(&app).await;
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
        Self::build_status(&app).await
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
        Self::build_status(&app).await
    }

    pub async fn runtime_status(&self) -> NodeResult<DesktopNodeStatusDto> {
        let app = self.inner.read().await;
        Self::build_status(&app).await
    }

    pub async fn workspace_status(&self) -> DesktopWorkspaceStatusDto {
        let app = self.inner.read().await;
        let config = &app.config;

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

    async fn build_status(app: &DesktopApp) -> NodeResult<DesktopNodeStatusDto> {
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

            return Ok(DesktopNodeStatusDto {
                name: node.config().name.clone(),
                node_id: Some(node.node_id().to_string()),
                lifecycle_state: app.lifecycle_state.clone(),
                connection_state,
                hub_url: node.config().connection.hub_url.clone(),
                workspace_path: node.config().workspace_path.clone(),
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

        Ok(DesktopNodeStatusDto {
            name: app.config.name.clone(),
            node_id: app.config.node_id.as_ref().map(ToString::to_string),
            lifecycle_state: app.lifecycle_state.clone(),
            connection_state: "disconnected".to_string(),
            hub_url: app.config.connection.hub_url.clone(),
            workspace_path: app.config.workspace_path.clone(),
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
    use tempfile::TempDir;

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

        let stopped = state.stop_node().await.unwrap();
        assert!(matches!(
            stopped.lifecycle_state,
            DesktopLifecycleState::Stopped
        ));

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
