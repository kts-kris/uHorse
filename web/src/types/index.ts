export interface AgentRuntimeSummary {
  agent_id: string;
  name: string;
  description: string;
  workspace_dir: string;
  is_default: boolean;
  skill_names: string[];
  active_session_count: number;
}

export interface AgentRuntimeDetail extends AgentRuntimeSummary {
  system_prompt: string;
}

export interface SkillRuntimeSummary {
  name: string;
  description: string;
  version: string;
  enabled: boolean;
  timeout_secs: number;
  max_retries: number;
  executable: string | null;
  args: string[];
  permissions: string[];
  execution_mode: string;
}

export interface SkillRuntimeDetail extends SkillRuntimeSummary {
  author: string | null;
  env: Record<string, string>;
}

export interface SessionRuntimeSummary {
  session_id: string;
  agent_id: string | null;
  conversation_id: string | null;
  sender_user_id: string | null;
  sender_staff_id: string | null;
  last_task_id: string | null;
  message_count: number;
  created_at: string;
  last_active: string;
}

export interface SessionRuntimeDetail extends SessionRuntimeSummary {
  metadata: Record<string, string>;
}

export interface SessionMessageRecord {
  timestamp: string;
  user_message: string;
  assistant_message: string;
}

export interface NodeManagerStats {
  total_nodes: number;
  online_nodes: number;
  offline_nodes: number;
  busy_nodes: number;
}

export interface SchedulerStats {
  pending_tasks: number;
  running_tasks: number;
  completed_tasks: number;
  failed_tasks: number;
}

export interface HubStats {
  hub_id: string;
  uptime_secs: number;
  nodes: NodeManagerStats;
  scheduler: SchedulerStats;
  updated_at: string;
}

export interface NodeCapabilitiesInfo {
  supported_commands: string[];
  tags: string[];
  max_concurrent_tasks: number;
  available_tools: string[];
}

export interface WorkspaceRuntimeInfo {
  name: string;
  path: string;
  read_only: boolean;
  allowed_patterns: string[];
  denied_patterns: string[];
}

export interface LoadInfo {
  cpu_usage: number;
  memory_usage: number;
  task_count: number;
  latency_ms: number | null;
}

export interface NodeRuntimeInfo {
  node_id: string;
  name: string;
  state: string;
  capabilities: NodeCapabilitiesInfo;
  workspace: WorkspaceRuntimeInfo;
  tags: string[];
  last_heartbeat: string;
  load: LoadInfo;
  registered_at: string;
  current_tasks: number;
  completed_tasks: number;
  failed_tasks: number;
}

export interface TaskRuntimeInfo {
  task_id: string;
  status: string;
  command_type: string;
  priority: string;
  started_at: string | null;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  token_type: string;
}

export interface FileInfo {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified_at: string;
}

export interface FileContent {
  path: string;
  content: string;
  encoding: string;
}

export interface MarketplaceSkill {
  id: string;
  name: string;
  description?: string;
  version: string;
  author?: string;
  downloads: number;
  rating: number;
  tags: string[];
}
