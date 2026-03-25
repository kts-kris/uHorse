//! # WebSocket 协议定义
//!
//! 定义 WebSocket 通信协议的消息格式和握手流程。

use crate::types::ErrorCode;
use serde::{Deserialize, Serialize};

/// WebSocket 协议消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ProtocolMessage {
    /// 握手请求
    Handshake(HandshakeRequest),
    /// 握手响应
    HandshakeResponse(HandshakeResponse),

    /// 请求 (Request-Response 模式)
    Request(Request),
    /// 响应
    Response(Response),

    /// 事件 (Event 模式)
    Event(Event),

    /// Ping
    Ping(Ping),
    /// Pong
    Pong(Pong),
}

impl ProtocolMessage {
    /// 获取消息类型名称
    pub fn type_name(&self) -> &'static str {
        match self {
            ProtocolMessage::Handshake(_) => "handshake",
            ProtocolMessage::HandshakeResponse(_) => "handshake_response",
            ProtocolMessage::Request(_) => "request",
            ProtocolMessage::Response(_) => "response",
            ProtocolMessage::Event(_) => "event",
            ProtocolMessage::Ping(_) => "ping",
            ProtocolMessage::Pong(_) => "pong",
        }
    }

    /// 是否为控制消息
    pub fn is_control(&self) -> bool {
        matches!(self, ProtocolMessage::Ping(_) | ProtocolMessage::Pong(_))
    }
}

// ============== 握手协议 ==============

/// 握手请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    /// 客户端版本
    pub version: semver::Version,
    /// 客户端能力
    pub capabilities: ClientCapabilities,
    /// 认证令牌
    pub auth_token: Option<String>,
    /// 设备 ID
    pub device_id: Option<String>,
}

/// 握手响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    /// 服务器版本
    pub server_version: semver::Version,
    /// 会话 ID
    pub session_id: String,
    /// 服务器能力
    pub capabilities: ServerCapabilities,
    /// 是否需要配对
    pub pairing_required: bool,
    /// 认证状态
    pub auth_status: AuthStatus,
}

/// 客户端能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// 最大消息大小
    pub max_message_size: usize,
    /// 支持的通道
    pub supported_channels: Vec<String>,
    /// 支持的压缩
    pub supports_compression: bool,
}

impl Default for ClientCapabilities {
    fn default() -> Self {
        Self {
            max_message_size: 65536,
            supported_channels: vec!["telegram".to_string(), "slack".to_string()],
            supports_compression: false,
        }
    }
}

/// 服务器能力声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// 最大消息大小
    pub max_message_size: usize,
    /// 支持的通道
    pub supported_channels: Vec<String>,
    /// 支持的工具
    pub supported_tools: Vec<String>,
    /// 服务器特性
    pub features: ServerFeatures,
}

/// 服务器特性声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerFeatures {
    /// 支持幂等性
    pub idempotency: bool,
    /// 支持事件流
    pub event_stream: bool,
    /// 支持批量请求
    pub batch_requests: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            max_message_size: 1048576, // 1MB
            supported_channels: vec!["telegram".to_string()],
            supported_tools: Vec::new(),
            features: ServerFeatures {
                idempotency: true,
                event_stream: true,
                batch_requests: false,
            },
        }
    }
}

/// 认证状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthStatus {
    /// 已认证
    Authenticated,
    /// 未认证
    Unauthenticated,
    /// 等待配对
    PendingPairing,
}

// ============== 请求-响应协议 ==============

/// 请求消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// 请求 ID（用于关联响应）
    pub id: String,
    /// 方法名
    pub method: String,
    /// 方法参数
    pub params: serde_json::Value,
    /// 幂等键（可选）
    pub idempotency_key: Option<String>,
}

impl Request {
    /// 创建新请求
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
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

/// 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// 请求 ID
    pub id: String,
    /// 结果（成功时）
    pub result: Option<serde_json::Value>,
    /// 错误（失败时）
    pub error: Option<ErrorDetail>,
}

impl Response {
    /// 创建成功响应
    pub fn ok(id: impl Into<String>, result: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    /// 创建错误响应
    pub fn err(id: impl Into<String>, error: ErrorDetail) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }

    /// 是否为成功响应
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// 错误详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// 错误码
    pub code: ErrorCode,
    /// 错误消息
    pub message: String,
    /// 错误详情（可选）
    pub details: Option<serde_json::Value>,
}

impl ErrorDetail {
    /// 创建新的错误详情
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

impl std::fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.message)
    }
}

// ============== 事件协议 ==============

/// 事件消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// 事件名称
    pub name: String,
    /// 事件数据
    pub data: serde_json::Value,
    /// 事件序号（用于一致性保证）
    pub sequence: u64,
}

impl Event {
    /// 创建新事件
    pub fn new(name: impl Into<String>, data: serde_json::Value, sequence: u64) -> Self {
        Self {
            name: name.into(),
            data,
            sequence,
        }
    }
}

/// 预定义的事件名称
pub mod events {
    /// 消息接收事件
    pub const MESSAGE_RECEIVED: &str = "message.received";
    /// 消息发送事件
    pub const MESSAGE_SENT: &str = "message.sent";
    /// 工具执行成功事件
    pub const TOOL_EXECUTED: &str = "tool.executed";
    /// 工具执行失败事件
    pub const TOOL_FAILED: &str = "tool.failed";
    /// 会话创建事件
    pub const SESSION_CREATED: &str = "session.created";
    /// 会话关闭事件
    pub const SESSION_CLOSED: &str = "session.closed";
    /// 设备连接事件
    pub const DEVICE_CONNECTED: &str = "device.connected";
    /// 设备断开事件
    pub const DEVICE_DISCONNECTED: &str = "device.disconnected";
}

// ============== 心跳协议 ==============

/// Ping 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    /// 时间戳
    pub timestamp: u64,
}

impl Ping {
    /// 创建新的 Ping 消息
    pub fn new() -> Self {
        Self {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl Default for Ping {
    fn default() -> Self {
        Self::new()
    }
}

/// Pong 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pong {
    /// 时间戳（回应 Ping 的时间戳）
    pub timestamp: u64,
}

impl Pong {
    /// 创建新的 Pong 消息
    pub fn new(timestamp: u64) -> Self {
        Self { timestamp }
    }
}

// ============== 协议常量 ==============

/// 协议版本
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// 默认心跳间隔（秒）
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;

/// 默认握手超时（秒）
pub const DEFAULT_HANDSHAKE_TIMEOUT: u64 = 10;

/// 默认消息大小限制（字节）
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
