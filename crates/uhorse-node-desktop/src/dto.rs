use serde::{Deserialize, Serialize};
use uhorse_node_runtime::NodeConfig;

use crate::config_store::DesktopPreferencesConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopSettingsDto {
    pub name: String,
    pub workspace_path: String,
    pub hub_url: String,
    pub require_git_repo: bool,
    pub watch_workspace: bool,
    pub git_protection_enabled: bool,
    pub auto_git_add_new_files: bool,
    pub notifications_enabled: bool,
    pub show_notification_details: bool,
    pub mirror_notifications_to_dingtalk: bool,
    pub launch_at_login: bool,
}

impl DesktopSettingsDto {
    pub fn from_config(config: &NodeConfig, desktop: &DesktopPreferencesConfig) -> Self {
        Self {
            name: config.name.clone(),
            workspace_path: config.workspace_path.clone(),
            hub_url: config.connection.hub_url.clone(),
            require_git_repo: config.require_git_repo,
            watch_workspace: config.watch_workspace,
            git_protection_enabled: config.git_protection_enabled,
            auto_git_add_new_files: config.auto_git_add_new_files,
            notifications_enabled: desktop.notifications_enabled,
            show_notification_details: desktop.show_notification_details,
            mirror_notifications_to_dingtalk: desktop.mirror_notifications_to_dingtalk,
            launch_at_login: desktop.launch_at_login,
        }
    }

    pub fn apply_to_config(&self, config: &mut NodeConfig, desktop: &mut DesktopPreferencesConfig) {
        config.name = self.name.clone();
        config.workspace_path = self.workspace_path.clone();
        config.connection.hub_url = self.hub_url.clone();
        config.require_git_repo = self.require_git_repo;
        config.watch_workspace = self.watch_workspace;
        config.git_protection_enabled = self.git_protection_enabled;
        config.auto_git_add_new_files = self.auto_git_add_new_files;
        desktop.notifications_enabled = self.notifications_enabled;
        desktop.show_notification_details = self.show_notification_details;
        desktop.mirror_notifications_to_dingtalk = self.mirror_notifications_to_dingtalk;
        desktop.launch_at_login = self.launch_at_login;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartAccountPairingRequest {
    pub node_id: String,
    pub node_name: String,
    pub device_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAccountPairingRequest {
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopPairingRequestDto {
    pub request_id: String,
    pub node_id: String,
    pub node_name: String,
    pub device_type: String,
    pub pairing_code: String,
    pub status: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub bound_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopAccountStatusDto {
    pub node_id: String,
    pub pairing_enabled: bool,
    pub bound_user_id: Option<String>,
    pub pairing: Option<DesktopPairingRequestDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceValidationRequest {
    pub workspace_path: String,
    pub require_git_repo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceValidationDto {
    pub valid: bool,
    pub path: String,
    pub normalized_path: Option<String>,
    pub name: Option<String>,
    pub git_repo: bool,
    pub require_git_repo: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopLifecycleState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopMetricsDto {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_duration_ms: f64,
    pub success_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopHeartbeatDto {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub disk_gb: f64,
    pub network_latency_ms: Option<u64>,
    pub last_heartbeat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopCheckpointDto {
    pub revision: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopNodeStatusDto {
    pub name: String,
    pub node_id: Option<String>,
    pub lifecycle_state: DesktopLifecycleState,
    pub connection_state: String,
    pub hub_url: String,
    pub workspace_path: String,
    pub saved_workspace_path: String,
    pub runtime_workspace_path: Option<String>,
    pub restart_required: bool,
    pub restart_notice: Option<String>,
    pub running_tasks: usize,
    pub max_concurrent_tasks: usize,
    pub pending_approvals: usize,
    pub metrics: DesktopMetricsDto,
    pub heartbeat: Option<DesktopHeartbeatDto>,
    pub latest_checkpoint: Option<DesktopCheckpointDto>,
    pub recent_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopWorkspaceStatusDto {
    pub valid: bool,
    pub name: Option<String>,
    pub path: String,
    pub normalized_path: Option<String>,
    pub read_only: bool,
    pub git_repo: bool,
    pub require_git_repo: bool,
    pub watch_workspace: bool,
    pub git_protection_enabled: bool,
    pub auto_git_add_new_files: bool,
    pub internal_work_dir: String,
    pub allowed_patterns: Vec<String>,
    pub denied_patterns: Vec<String>,
    pub running_workspace_path: Option<String>,
    pub restart_required: bool,
    pub restart_notice: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopVersionEntryDto {
    pub path: String,
    pub staged_status: String,
    pub unstaged_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopVersionSummaryDto {
    pub available: bool,
    pub error: Option<String>,
    pub branch: Option<String>,
    pub dirty: bool,
    pub entries: Vec<DesktopVersionEntryDto>,
    pub current_checkpoint: Option<DesktopCheckpointDto>,
    pub checkpoints: Vec<DesktopCheckpointDto>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DesktopLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopLogEntryDto {
    pub level: DesktopLogLevel,
    pub message: String,
    pub timestamp: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultSettingsDto {
    pub suggested_name: String,
    pub notifications_enabled: bool,
    pub show_notification_details: bool,
    pub mirror_notifications_to_dingtalk: bool,
    pub launch_at_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryPickerResponseDto {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopCapabilityStatusDto {
    pub notifications_enabled: bool,
    pub show_notification_details: bool,
    pub mirror_notifications_to_dingtalk: bool,
    pub launch_at_login: bool,
    pub launch_agent_installed: bool,
    pub launch_agent_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_status_dto_deserializes_pairing_payload() {
        let json = serde_json::json!({
            "node_id": "node-desktop-test",
            "pairing_enabled": true,
            "bound_user_id": "ding-user-1",
            "pairing": {
                "request_id": "req-1",
                "node_id": "node-desktop-test",
                "node_name": "Desktop Node",
                "device_type": "desktop",
                "pairing_code": "123456",
                "status": "pending",
                "created_at": 1,
                "expires_at": 2,
                "bound_user_id": null
            }
        });

        let dto: DesktopAccountStatusDto = serde_json::from_value(json).unwrap();
        assert_eq!(dto.node_id, "node-desktop-test");
        assert!(dto.pairing_enabled);
        assert_eq!(dto.bound_user_id.as_deref(), Some("ding-user-1"));
        assert_eq!(
            dto.pairing
                .as_ref()
                .map(|value| value.pairing_code.as_str()),
            Some("123456")
        );
    }

    #[test]
    fn test_desktop_settings_round_trip_preserves_desktop_preferences() {
        let mut config = NodeConfig::default();
        config.name = "Desktop Node".to_string();
        config.workspace_path = "/tmp/workspace".to_string();
        config.connection.hub_url = "ws://localhost:8765/ws".to_string();
        config.require_git_repo = false;
        config.watch_workspace = false;
        config.git_protection_enabled = false;
        config.auto_git_add_new_files = false;

        let desktop = DesktopPreferencesConfig {
            notifications_enabled: true,
            show_notification_details: false,
            mirror_notifications_to_dingtalk: true,
            launch_at_login: true,
        };

        let dto = DesktopSettingsDto::from_config(&config, &desktop);
        assert!(dto.mirror_notifications_to_dingtalk);

        let mut next_config = NodeConfig::default();
        let mut next_desktop = DesktopPreferencesConfig::default();
        dto.apply_to_config(&mut next_config, &mut next_desktop);

        assert_eq!(next_config.name, "Desktop Node");
        assert_eq!(next_config.workspace_path, "/tmp/workspace");
        assert_eq!(next_config.connection.hub_url, "ws://localhost:8765/ws");
        assert!(!next_config.require_git_repo);
        assert!(!next_config.watch_workspace);
        assert!(!next_config.git_protection_enabled);
        assert!(!next_config.auto_git_add_new_files);
        assert!(next_desktop.notifications_enabled);
        assert!(!next_desktop.show_notification_details);
        assert!(next_desktop.mirror_notifications_to_dingtalk);
        assert!(next_desktop.launch_at_login);
    }
}
