//! # uHorse Core Types
//!
//! 定义系统的核心数据结构，包括会话、消息、工具等。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

// ============== 会话类型 ==============

/// 会话唯一标识符
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    /// 生成新的会话 ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// 使用已有字符串创建会话 ID
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取内部字符串表示
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
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, strum::Display, strum::EnumIter,
)]
pub enum ChannelType {
    /// Telegram 通道
    Telegram,
    /// Slack 通道
    Slack,
    /// Discord 通道
    Discord,
    /// WhatsApp 通道
    WhatsApp,
    /// DingTalk 通道
    DingTalk,
    /// Feishu 通道
    Feishu,
    /// WeWork 通道
    WeWork,
}

impl ChannelType {
    /// 从字符串解析
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        <Self as FromStr>::from_str(s).ok()
    }
}

impl FromStr for ChannelType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "telegram" => Ok(ChannelType::Telegram),
            "slack" => Ok(ChannelType::Slack),
            "discord" => Ok(ChannelType::Discord),
            "whatsapp" => Ok(ChannelType::WhatsApp),
            "dingtalk" | "钉钉" => Ok(ChannelType::DingTalk),
            "feishu" | "飞书" => Ok(ChannelType::Feishu),
            "wework" | "企业微信" | "wecom" => Ok(ChannelType::WeWork),
            _ => Err(()),
        }
    }
}

// ============== 消息类型 ==============

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 系统消息
    System,
    /// 工具消息
    Tool,
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    /// 纯文本内容
    Text(String),
    /// 图片内容
    Image {
        /// 图片地址
        url: String,
        /// 图片说明
        caption: Option<String>,
    },
    /// 音频内容
    Audio {
        /// 音频地址
        url: String,
        /// 音频时长（秒）
        duration: Option<u32>,
    },
    /// 结构化内容
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
    pub fn new(
        session_id: SessionId,
        role: MessageRole,
        content: MessageContent,
        sequence: u64,
    ) -> Self {
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
    /// 使用指定字符串创建工具 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 获取内部字符串表示
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
    /// 创建新的工具调用请求
    pub fn new(tool_id: ToolId, params: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tool_id,
            params,
            idempotency_key: None,
        }
    }

    /// 设置幂等键
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
    /// 创建新的工具错误
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// 设置错误详情
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// 执行上下文
#[derive(Debug, Clone, Default)]
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

// ============== 调度类型 ==============

/// 调度任务 ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct JobId(pub String);

impl JobId {
    /// 生成新的调度任务 ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// 获取内部字符串表示
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
    At {
        /// 执行时间（Unix 时间戳，秒）
        time: u64,
    },
    /// 每隔指定秒数执行
    Every {
        /// 执行间隔（秒）
        duration_secs: u64,
    },
    /// Cron 表达式
    Cron {
        /// Cron 表达式字符串
        expression: String,
    },
}

/// 调度目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobTarget {
    /// 调用工具
    Tool {
        /// 目标工具 ID
        id: ToolId,
        /// 工具参数
        params: serde_json::Value,
    },
    /// 调用插件方法
    Plugin {
        /// 插件名称
        name: String,
        /// 插件方法名
        method: String,
        /// 插件参数
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
    /// 生成新的设备 ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// 使用已有字符串创建设备 ID
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取内部字符串表示
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
    /// 未知错误
    Unknown = 1,
    /// 内部错误
    InternalError = 2,
    /// 功能尚未实现
    NotImplemented = 3,

    // ============== 协议错误 (100-199) ==============
    /// 消息格式无效
    InvalidMessage = 100,
    /// 协议版本不受支持
    UnsupportedVersion = 101,
    /// 握手失败
    HandshakeFailed = 102,

    // ============== 认证错误 (200-299) ==============
    /// 未授权访问
    Unauthorized = 200,
    /// 令牌已过期
    TokenExpired = 201,
    /// 令牌无效
    InvalidToken = 202,
    /// 设备未配对
    DeviceNotPaired = 203,

    // ============== 会话错误 (300-399) ==============
    /// 会话不存在
    SessionNotFound = 300,
    /// 会话已过期
    SessionExpired = 301,
    /// 会话已隔离
    SessionIsolated = 302,

    // ============== 工具错误 (400-499) ==============
    /// 工具不存在
    ToolNotFound = 400,
    /// 工具权限不足
    ToolPermissionDenied = 401,
    /// 工具参数校验失败
    ToolValidationFailed = 402,
    /// 工具执行失败
    ToolExecutionFailed = 403,

    // ============== 调度错误 (500-599) ==============
    /// 调度任务不存在
    JobNotFound = 500,
    /// 调度任务执行失败
    JobExecutionFailed = 501,
    /// 调度规则冲突
    ScheduleConflict = 502,

    // ============== 幂等性错误 (600-699) ==============
    /// 幂等键冲突
    IdempotencyConflict = 600,
    /// 幂等键已过期
    IdempotencyKeyExpired = 601,
}

impl ErrorCode {
    /// 获取错误类别
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::Unknown | ErrorCode::InternalError | ErrorCode::NotImplemented => {
                ErrorCategory::General
            }
            ErrorCode::InvalidMessage
            | ErrorCode::UnsupportedVersion
            | ErrorCode::HandshakeFailed => ErrorCategory::Protocol,
            ErrorCode::Unauthorized
            | ErrorCode::TokenExpired
            | ErrorCode::InvalidToken
            | ErrorCode::DeviceNotPaired => ErrorCategory::Auth,
            ErrorCode::SessionNotFound | ErrorCode::SessionExpired | ErrorCode::SessionIsolated => {
                ErrorCategory::Session
            }
            ErrorCode::ToolNotFound
            | ErrorCode::ToolPermissionDenied
            | ErrorCode::ToolValidationFailed
            | ErrorCode::ToolExecutionFailed => ErrorCategory::Tool,
            ErrorCode::JobNotFound
            | ErrorCode::JobExecutionFailed
            | ErrorCode::ScheduleConflict => ErrorCategory::Scheduler,
            ErrorCode::IdempotencyConflict | ErrorCode::IdempotencyKeyExpired => {
                ErrorCategory::Idempotency
            }
        }
    }

    /// 是否为客户端错误 (4xx)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::InvalidMessage
                | ErrorCode::Unauthorized
                | ErrorCode::TokenExpired
                | ErrorCode::InvalidToken
                | ErrorCode::DeviceNotPaired
                | ErrorCode::SessionNotFound
                | ErrorCode::ToolNotFound
                | ErrorCode::ToolPermissionDenied
                | ErrorCode::ToolValidationFailed
        )
    }

    /// 是否为服务端错误 (5xx)
    pub fn is_server_error(&self) -> bool {
        !self.is_client_error()
    }
}

/// 错误类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 通用错误
    General,
    /// 协议错误
    Protocol,
    /// 认证错误
    Auth,
    /// 会话错误
    Session,
    /// 工具错误
    Tool,
    /// 调度错误
    Scheduler,
    /// 幂等性错误
    Idempotency,
}
