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

pub mod types;
pub mod protocol;
pub mod error;
pub mod traits;

// 重新导出常用类型
pub use types::{
    SessionId,
    Session,
    IsolationLevel,
    ChannelType,
    Message,
    MessageContent,
    MessageRole,
    ToolId,
    Tool,
    ToolCall,
    ToolResult,
    ToolError,
    ExecutionContext,
    JobId,
    Schedule,
    JobTarget,
    ScheduledJob,
    DeviceId,
    DeviceInfo,
    DeviceCapabilities,
    AccessToken,
    ErrorCode,
    ErrorCategory,
    PermissionLevel,
};

pub use protocol::{
    ProtocolMessage,
    HandshakeRequest,
    HandshakeResponse,
    Request,
    Response,
    Event,
    ErrorDetail,
    Ping,
    Pong,
    ClientCapabilities,
    ServerCapabilities,
    ServerFeatures,
    AuthStatus,
    events,
    PROTOCOL_VERSION,
    DEFAULT_HEARTBEAT_INTERVAL,
    DEFAULT_HANDSHAKE_TIMEOUT,
    DEFAULT_MAX_MESSAGE_SIZE,
};

pub use error::{
    UHorseError,
    Result,
    ChannelError,
    PluginError,
    StorageError,
};

pub use traits::{
    Channel,
    ToolExecutor,
    Plugin,
    SessionStore,
    ConversationStore,
    ToolRegistry,
    DeviceManager,
    Scheduler,
    AuthService,
    IdempotencyService,
    EventBus,
};

// 依赖版本检查
// serde 和 async features 默认启用，不需要编译时检查

/// uHorse 核心版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
