export type ApiError =
  | string
  | {
      code?: string;
      message?: string;
      details?: unknown;
    };

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  error?: ApiError;
}

export type DesktopLifecycleState = 'stopped' | 'starting' | 'running' | 'stopping' | 'failed';

export interface DesktopSettings {
  name: string;
  workspace_path: string;
  hub_url: string;
  require_git_repo: boolean;
  watch_workspace: boolean;
  git_protection_enabled: boolean;
  auto_git_add_new_files: boolean;
  notifications_enabled: boolean;
  show_notification_details: boolean;
  mirror_notifications_to_dingtalk: boolean;
  launch_at_login: boolean;
}

export interface CancelAccountPairingRequest {
  request_id: string;
}

export interface DesktopPairingRequest {
  request_id: string;
  node_id: string;
  node_name: string;
  device_type: string;
  pairing_code: string;
  status: string;
  created_at: number;
  expires_at: number;
  bound_user_id: string | null;
}

export interface DesktopAccountStatus {
  node_id: string;
  pairing_enabled: boolean;
  bound_user_id: string | null;
  pairing: DesktopPairingRequest | null;
}

export interface DefaultSettings {
  suggested_name: string;
  notifications_enabled: boolean;
  show_notification_details: boolean;
  mirror_notifications_to_dingtalk: boolean;
  launch_at_login: boolean;
}

export interface DesktopCapabilityStatus {
  notifications_enabled: boolean;
  show_notification_details: boolean;
  mirror_notifications_to_dingtalk: boolean;
  launch_at_login: boolean;
  launch_agent_installed: boolean;
  launch_agent_path: string | null;
}

export interface DirectoryPickerResponse {
  path: string;
}

export interface WorkspaceValidationRequest {
  workspace_path: string;
  require_git_repo: boolean;
}

export interface WorkspaceValidation {
  valid: boolean;
  path: string;
  normalized_path: string | null;
  name: string | null;
  git_repo: boolean;
  require_git_repo: boolean;
  error: string | null;
}

export interface DesktopMetrics {
  total_executions: number;
  successful_executions: number;
  failed_executions: number;
  avg_duration_ms: number;
  success_rate: number;
}

export interface DesktopHeartbeat {
  cpu_percent: number;
  memory_mb: number;
  disk_gb: number;
  network_latency_ms: number | null;
  last_heartbeat: string;
}

export interface DesktopCheckpoint {
  revision: string;
  message: string;
  created_at: string;
}

export interface DesktopNodeStatus {
  name: string;
  node_id: string | null;
  lifecycle_state: DesktopLifecycleState;
  connection_state: string;
  hub_url: string;
  workspace_path: string;
  saved_workspace_path: string;
  runtime_workspace_path: string | null;
  restart_required: boolean;
  restart_notice: string | null;
  running_tasks: number;
  max_concurrent_tasks: number;
  pending_approvals: number;
  metrics: DesktopMetrics;
  heartbeat: DesktopHeartbeat | null;
  latest_checkpoint: DesktopCheckpoint | null;
  recent_error: string | null;
}

export interface DesktopWorkspaceStatus {
  valid: boolean;
  name: string | null;
  path: string;
  normalized_path: string | null;
  read_only: boolean;
  git_repo: boolean;
  require_git_repo: boolean;
  watch_workspace: boolean;
  git_protection_enabled: boolean;
  auto_git_add_new_files: boolean;
  internal_work_dir: string;
  allowed_patterns: string[];
  denied_patterns: string[];
  running_workspace_path: string | null;
  restart_required: boolean;
  restart_notice: string | null;
  error: string | null;
}

export interface DesktopVersionEntry {
  path: string;
  staged_status: string;
  unstaged_status: string;
}

export interface DesktopVersionSummary {
  available: boolean;
  error: string | null;
  branch: string | null;
  dirty: boolean;
  entries: DesktopVersionEntry[];
  current_checkpoint: DesktopCheckpoint | null;
  checkpoints: DesktopCheckpoint[];
}

export type DesktopLogLevel = 'INFO' | 'WARN' | 'ERROR';

export interface DesktopLogEntry {
  level: DesktopLogLevel;
  message: string;
  timestamp: string;
  source: string;
}
