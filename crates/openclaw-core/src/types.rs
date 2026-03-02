//! # OpenClaw Core Types
//!
//! 定义系统的核心数据结构，包括会话、消息、工具等。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

// ============== 会话类型 ==============

/// 会话唯一标识符
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    /// 生成新的会话 ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// 隔离级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    /// 无隔离，所有会话共享上下文
    #[default]
    None = 0,
    /// 按通道隔离
    Channel = 1,
    /// 按用户隔离
    User = 2,
    /// 完全隔离，每个会话独立
    Full = 3,
}

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 会话唯一 ID
    pub id: SessionId,
    /// 通道类型
    pub channel: ChannelType,
    /// 通道内的用户 ID
    pub channel_user_id: String,
    /// 创建时间 (Unix 时间戳，秒)
    pub created_at: u64,
    /// 最后更新时间 (Unix 时间戳，秒)
    pub updated_at: u64,
    /// 元数据
    pub metadata: HashMap<String, String>,
    /// 隔离级别
    pub isolation_level: IsolationLevel,
}

impl Session {
    /// 创建新会话
    pub fn new(channel: ChannelType, channel_user_id: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: SessionId::new(),
            channel,
            channel_user_id,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
            isolation_level: IsolationLevel::default(),
        }
    }

    /// 更新会话时间戳
    pub fn touch(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

// ============== 通道类型 ==============

/// 支持的消息通道类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub enum ChannelType {
    Telegram,
    Slack,
    Discord,
    WhatsApp,
}

impl ChannelType {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "telegram" => Some(ChannelType::Telegram),
            "slack" => Some(ChannelType::Slack),
            "discord" => Some(ChannelType::Discord),
            "whatsapp" => Some(ChannelType::WhatsApp),
            _ => None,
        }
    }
}

// ============== 消息类型 ==============

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    Image {
        url: String,
        caption: Option<String>,
    },
    Audio {
        url: String,
        duration: Option<u32>,
    },
    Structured(serde_json::Value),
}

impl MessageContent {
    /// 创建文本消息
    pub fn text(text: impl Into<String>) -> Self {
        MessageContent::Text(text.into())
    }

    /// 提取文本内容（如果存在）
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            _ => None,
        }
    }
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息唯一 ID
    pub id: String,
    /// 所属会话 ID
    pub session_id: SessionId,
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: MessageContent,
    /// 时间戳 (Unix 时间戳，秒)
    pub timestamp: u64,
    /// 序列号（用于事件一致性）
    pub sequence: u64,
}

impl Message {
    /// 创建新消息
    pub fn new(session_id: SessionId, role: MessageRole, content: MessageContent, sequence: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            role,
            content,
            timestamp: now,
            sequence,
        }
    }
}

// ============== 工具类型 ==============

/// 工具 ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ToolId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 权限级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PermissionLevel {
    /// 公开，无需认证
    #[default]
    Public = 0,
    /// 需要认证
    Authenticated = 1,
    /// 受信任用户
    Trusted = 2,
    /// 管理员
    Admin = 3,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// 工具 ID
    pub id: ToolId,
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: serde_json::Value,
    /// 权限级别
    pub permission_level: PermissionLevel,
    /// 是否启用
    pub enabled: bool,
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 调用 ID（用于关联请求和响应）
    pub id: String,
    /// 工具 ID
    pub tool_id: ToolId,
    /// 调用参数
    pub params: serde_json::Value,
    /// 幂等键（可选）
    pub idempotency_key: Option<String>,
}

impl ToolCall {
    pub fn new(tool_id: ToolId, params: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tool_id,
            params,
            idempotency_key: None,
        }
    }

    pub fn with_idempotency(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 调用 ID
    pub call_id: String,
    /// 执行结果
    pub result: Result<serde_json::Value, ToolError>,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
}

/// 工具执行错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    /// 错误码
    pub code: ErrorCode,
    /// 错误消息
    pub message: String,
    /// 错误详情
    pub details: Option<serde_json::Value>,
}

impl ToolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// 执行上下文
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 会话 ID
    pub session_id: SessionId,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 设备 ID
    pub device_id: Option<DeviceId>,
    /// 权限范围
    pub scopes: Vec<String>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            session_id: SessionId::default(),
            user_id: None,
            device_id: None,
            scopes: Vec::new(),
        }
    }
}

// ============== 调度类型 ==============

/// 调度任务 ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct JobId(pub String);

impl JobId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for JobId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

/// 调度表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Schedule {
    /// 在指定时间执行一次
    At { time: u64 },
    /// 每隔指定秒数执行
    Every { duration_secs: u64 },
    /// Cron 表达式
    Cron { expression: String },
}

/// 调度目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobTarget {
    /// 调用工具
    Tool {
        id: ToolId,
        params: serde_json::Value,
    },
    /// 调用插件方法
    Plugin {
        name: String,
        method: String,
        params: serde_json::Value,
    },
}

/// 调度任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    /// 任务 ID
    pub id: JobId,
    /// 任务名称
    pub name: String,
    /// 调度表达式
    pub schedule: Schedule,
    /// 执行目标
    pub target: JobTarget,
    /// 是否启用
    pub enabled: bool,
    /// 下次执行时间
    pub next_run: Option<u64>,
    /// 上次执行时间
    pub last_run: Option<u64>,
}

// ============== 认证类型 ==============

/// 设备 ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

/// 设备能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// 最大消息大小
    pub max_message_size: usize,
    /// 支持的工具列表
    pub supported_tools: Vec<ToolId>,
    /// 支持的通道列表
    pub supported_channels: Vec<ChannelType>,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            max_message_size: 65536, // 64KB
            supported_tools: Vec::new(),
            supported_channels: vec![ChannelType::Telegram],
        }
    }
}

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// 设备 ID
    pub id: DeviceId,
    /// 设备名称
    pub name: String,
    /// 是否已配对
    pub paired: bool,
    /// 配对时间
    pub paired_at: Option<u64>,
    /// 最后活跃时间
    pub last_seen: u64,
    /// 设备能力
    pub capabilities: DeviceCapabilities,
}

/// 访问令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    /// 令牌字符串
    pub token: String,
    /// 关联的设备 ID
    pub device_id: Option<DeviceId>,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 权限范围
    pub scopes: Vec<String>,
    /// 过期时间
    pub expires_at: u64,
    /// 创建时间
    pub created_at: u64,
}

// ============== 错误码 ==============

/// 统一错误码
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    // ============== 通用错误 (1-99) ==============
    Unknown = 1,
    InternalError = 2,
    NotImplemented = 3,

    // ============== 协议错误 (100-199) ==============
    InvalidMessage = 100,
    UnsupportedVersion = 101,
    HandshakeFailed = 102,

    // ============== 认证错误 (200-299) ==============
    Unauthorized = 200,
    TokenExpired = 201,
    InvalidToken = 202,
    DeviceNotPaired = 203,

    // ============== 会话错误 (300-399) ==============
    SessionNotFound = 300,
    SessionExpired = 301,
    SessionIsolated = 302,

    // ============== 工具错误 (400-499) ==============
    ToolNotFound = 400,
    ToolPermissionDenied = 401,
    ToolValidationFailed = 402,
    ToolExecutionFailed = 403,

    // ============== 调度错误 (500-599) ==============
    JobNotFound = 500,
    JobExecutionFailed = 501,
    ScheduleConflict = 502,

    // ============== 幂等性错误 (600-699) ==============
    IdempotencyConflict = 600,
    IdempotencyKeyExpired = 601,
}

impl ErrorCode {
    /// 获取错误类别
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::Unknown | ErrorCode::InternalError | ErrorCode::NotImplemented => ErrorCategory::General,
            ErrorCode::InvalidMessage | ErrorCode::UnsupportedVersion | ErrorCode::HandshakeFailed => ErrorCategory::Protocol,
            ErrorCode::Unauthorized | ErrorCode::TokenExpired | ErrorCode::InvalidToken | ErrorCode::DeviceNotPaired => ErrorCategory::Auth,
            ErrorCode::SessionNotFound | ErrorCode::SessionExpired | ErrorCode::SessionIsolated => ErrorCategory::Session,
            ErrorCode::ToolNotFound | ErrorCode::ToolPermissionDenied | ErrorCode::ToolValidationFailed | ErrorCode::ToolExecutionFailed => ErrorCategory::Tool,
            ErrorCode::JobNotFound | ErrorCode::JobExecutionFailed | ErrorCode::ScheduleConflict => ErrorCategory::Scheduler,
            ErrorCode::IdempotencyConflict | ErrorCode::IdempotencyKeyExpired => ErrorCategory::Idempotency,
        }
    }

    /// 是否为客户端错误 (4xx)
    pub fn is_client_error(&self) -> bool {
        matches!(self, ErrorCode::InvalidMessage | ErrorCode::Unauthorized | ErrorCode::TokenExpired | ErrorCode::InvalidToken | ErrorCode::DeviceNotPaired | ErrorCode::SessionNotFound | ErrorCode::ToolNotFound | ErrorCode::ToolPermissionDenied | ErrorCode::ToolValidationFailed)
    }

    /// 是否为服务端错误 (5xx)
    pub fn is_server_error(&self) -> bool {
        !self.is_client_error()
    }
}

/// 错误类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    General,
    Protocol,
    Auth,
    Session,
    Tool,
    Scheduler,
    Idempotency,
}
