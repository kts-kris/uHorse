// Agent 相关类型
export interface Agent {
  id: string;
  name: string;
  description?: string;
  model: string;
  system_prompt?: string;
  temperature: number;
  max_tokens: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateAgentRequest {
  name: string;
  description?: string;
  model: string;
  system_prompt?: string;
  temperature?: number;
  max_tokens?: number;
}

export interface UpdateAgentRequest {
  name?: string;
  description?: string;
  model?: string;
  system_prompt?: string;
  temperature?: number;
  max_tokens?: number;
  enabled?: boolean;
}

// Skill 相关类型
export interface SkillParameter {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'object' | 'array';
  description?: string;
  required: boolean;
  default?: unknown;
}

export interface Skill {
  id: string;
  name: string;
  description?: string;
  version: string;
  author?: string;
  parameters: SkillParameter[];
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateSkillRequest {
  name: string;
  description?: string;
  version?: string;
  author?: string;
  parameters?: SkillParameter[];
  skill_content: string;
}

// Session 相关类型
export type SessionStatus = 'active' | 'paused' | 'closed';
export type MessageRole = 'user' | 'assistant' | 'system' | 'tool';

export interface SessionMessage {
  id: string;
  session_id: string;
  role: MessageRole;
  content: string;
  created_at: string;
}

export interface Session {
  id: string;
  agent_id: string;
  channel_type: string;
  status: SessionStatus;
  created_at: string;
  updated_at: string;
}

// 系统相关类型
export interface SystemInfo {
  name: string;
  version: string;
  uptime_secs: number;
  rust_version: string;
  channels_count: number;
  agents_count: number;
  active_sessions: number;
}

export interface SystemMetrics {
  total_messages: number;
  messages_today: number;
  total_requests: number;
  total_errors: number;
  avg_response_time_ms: number;
  memory_usage_bytes: number;
}

// Channel 相关类型
export interface ChannelStatus {
  channel_type: string;
  enabled: boolean;
  running: boolean;
  connected: boolean;
  last_activity?: string;
  error?: string;
}

// Auth 相关类型
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

// 文件相关类型
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

// 技能市场相关类型
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
