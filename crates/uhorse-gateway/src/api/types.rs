//! # API Types
//!
//! 定义 REST API 的请求和响应类型。

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ============================================================================
// 通用响应类型
// ============================================================================

/// 通用 API 响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    /// 是否成功
    pub success: bool,
    /// 响应数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// 创建错误响应
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

/// API 错误
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
}

// ============================================================================
// 分页
// ============================================================================

/// 分页查询参数
#[derive(Debug, Deserialize, ToSchema)]
pub struct PaginationQuery {
    /// 页码（从 1 开始）
    #[serde(default = "default_page")]
    pub page: u32,
    /// 每页数量
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

/// 分页响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// 数据列表
    pub items: Vec<T>,
    /// 总数
    pub total: u64,
    /// 当前页
    pub page: u32,
    /// 每页数量
    pub per_page: u32,
    /// 总页数
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    /// 创建分页响应
    pub fn new(items: Vec<T>, total: u64, page: u32, per_page: u32) -> Self {
        let total_pages = if per_page > 0 {
            ((total as f64) / (per_page as f64)).ceil() as u32
        } else {
            0
        };
        Self {
            items,
            total,
            page,
            per_page,
            total_pages,
        }
    }
}

// ============================================================================
// Agent DTOs
// ============================================================================

/// Agent 响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentDto {
    /// Agent ID
    pub id: String,
    /// 名称
    pub name: String,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 系统提示词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// 使用的模型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// 最大 token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 是否为默认 Agent
    pub is_default: bool,
    /// 技能列表
    #[serde(default)]
    pub skills: Vec<String>,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// 创建 Agent 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    /// 名称
    pub name: String,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 系统提示词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// 使用的模型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// 最大 token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 是否为默认 Agent
    #[serde(default)]
    pub is_default: bool,
    /// 技能列表
    #[serde(default)]
    pub skills: Vec<String>,
}

/// 更新 Agent 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAgentRequest {
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 系统提示词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// 使用的模型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// 最大 token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 是否为默认 Agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    /// 技能列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
}

// ============================================================================
// Skill DTOs
// ============================================================================

/// Skill 响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SkillDto {
    /// Skill ID
    pub id: String,
    /// 名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 参数定义
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    /// 所属 Agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// Skill 参数
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct SkillParameter {
    /// 参数名
    pub name: String,
    /// 描述
    pub description: String,
    /// 类型
    #[serde(rename = "type")]
    pub parameter_type: String,
    /// 是否必填
    pub required: bool,
    /// 默认值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// 创建 Skill 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSkillRequest {
    /// 名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 参数定义
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    /// SKILL.md 内容
    pub manifest: String,
    /// 所属 Agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// 更新 Skill 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSkillRequest {
    /// 名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 版本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// 参数定义
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<SkillParameter>>,
    /// SKILL.md 内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
}

// ============================================================================
// Session DTOs
// ============================================================================

/// Session 响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SessionDto {
    /// Session ID
    pub id: String,
    /// Agent ID
    pub agent_id: String,
    /// 通道类型
    pub channel: String,
    /// 用户 ID
    pub user_id: String,
    /// 状态
    pub status: SessionStatus,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// Session 状态
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// 活跃
    Active,
    /// 空闲
    Idle,
    /// 已关闭
    Closed,
}

/// Session 消息
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SessionMessageDto {
    /// 消息 ID
    pub id: String,
    /// 角色
    pub role: MessageRole,
    /// 内容
    pub content: String,
    /// 时间戳
    pub timestamp: String,
}

/// 消息角色
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// 用户
    User,
    /// 助手
    Assistant,
    /// 系统
    System,
}

// ============================================================================
// File DTOs
// ============================================================================

/// 文件信息
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FileInfo {
    /// 文件路径
    pub path: String,
    /// 文件名
    pub name: String,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件大小（字节）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// 修改时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
}

/// 文件内容
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FileContent {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
    /// 文件大小
    pub size: u64,
}

/// 创建文件请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFileRequest {
    /// 文件内容
    pub content: String,
}

/// 更新文件请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFileRequest {
    /// 文件内容
    pub content: String,
}

// ============================================================================
// Auth DTOs
// ============================================================================

/// 登录请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

/// Token 响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 过期时间（秒）
    pub expires_in: u64,
    /// 令牌类型
    pub token_type: String,
}

impl Default for TokenResponse {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            refresh_token: String::new(),
            expires_in: 86400, // 24 hours
            token_type: "Bearer".to_string(),
        }
    }
}

/// 刷新令牌请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    /// 刷新令牌
    pub refresh_token: String,
}

// ============================================================================
// Channel DTOs
// ============================================================================

/// 通道状态响应
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChannelStatusDto {
    /// 通道类型
    pub channel_type: String,
    /// 是否启用
    pub enabled: bool,
    /// 是否运行中
    pub running: bool,
    /// 连接状态
    pub connected: bool,
    /// 最后活动时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// System DTOs
// ============================================================================

/// 系统信息
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemInfo {
    /// 服务名称
    pub name: String,
    /// 版本
    pub version: String,
    /// 运行时间（秒）
    pub uptime_secs: u64,
    /// Go 版本（Rust 版本）
    pub rust_version: String,
    /// 通道数量
    pub channels_count: usize,
    /// Agent 数量
    pub agents_count: usize,
    /// 活跃会话数
    pub active_sessions: usize,
}

/// 系统指标
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemMetrics {
    /// 消息总数
    pub total_messages: u64,
    /// 今日消息数
    pub messages_today: u64,
    /// 请求总数
    pub total_requests: u64,
    /// 错误总数
    pub total_errors: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 内存使用（字节）
    pub memory_usage_bytes: u64,
}

// ============================================================================
// Marketplace DTOs
// ============================================================================

/// 市场技能
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceSkill {
    /// Skill ID
    pub id: String,
    /// 名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    pub author: String,
    /// 下载量
    pub downloads: u64,
    /// 评分
    pub rating: f32,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 图标 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// 仓库 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

/// 市场搜索查询
#[derive(Debug, Deserialize, ToSchema)]
pub struct MarketplaceSearchQuery {
    /// 搜索关键词
    pub q: Option<String>,
    /// 标签过滤
    #[serde(default)]
    pub tags: Vec<String>,
    /// 排序方式
    #[serde(default)]
    pub sort: Option<String>,
}

/// 安装技能请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct InstallSkillRequest {
    /// 目标 Agent ID
    pub agent_id: Option<String>,
}
