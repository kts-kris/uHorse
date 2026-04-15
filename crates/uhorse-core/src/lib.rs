//! # uHorse Core
//!
//! 多渠道 AI 网关框架的核心类型和 trait 定义。
//!
//! ## 模块结构
//!
//! - `types`: 核心数据类型（会话、消息、工具等）
//! - `protocol`: WebSocket 协议定义
//! - `error`: 统一错误类型
//! - `traits`: 可扩展的 trait 接口

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod protocol;
pub mod traits;
pub mod types;

// 重新导出常用类型
pub use types::{
    AccessToken, ChannelCapabilityFlags, ChannelRecipient, ChannelType, DeviceCapabilities,
    DeviceId, DeviceInfo, ErrorCategory, ErrorCode, ExecutionContext, IsolationLevel, JobId,
    JobTarget, Message, MessageContent, MessageRole, PermissionLevel, ReplyContext, Schedule,
    ScheduledJob, Session, SessionId, Tool, ToolCall, ToolError, ToolId, ToolResult,
};

pub use protocol::{
    events, AuthStatus, ClientCapabilities, ErrorDetail, Event, HandshakeRequest,
    HandshakeResponse, Ping, Pong, ProtocolMessage, Request, Response, ServerCapabilities,
    ServerFeatures, DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_HEARTBEAT_INTERVAL,
    DEFAULT_MAX_MESSAGE_SIZE, PROTOCOL_VERSION,
};

pub use error::{ChannelError, PluginError, Result, StorageError, UHorseError};

pub use traits::{
    AuthService, Channel, ConversationStore, DeviceManager, EventBus, IdempotencyService, Plugin,
    Scheduler, SessionStore, ToolExecutor, ToolRegistry,
};

// 依赖版本检查
// serde 和 async features 默认启用，不需要编译时检查

/// uHorse 核心版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
