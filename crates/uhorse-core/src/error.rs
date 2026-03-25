//! # 统一错误定义
//!
//! 定义系统所有错误类型。

use crate::types::{DeviceId, ErrorCode, JobId, SessionId, ToolId};
use thiserror::Error;

/// uHorse 统一错误类型
#[derive(Error, Debug)]
pub enum UHorseError {
    // ============== 会话错误 ==============
    /// 指定会话不存在
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    /// 指定会话已过期
    #[error("Session expired: {0}")]
    SessionExpired(SessionId),

    /// 指定会话已被隔离
    #[error("Session isolated: {0}")]
    SessionIsolated(SessionId),

    // ============== 工具错误 ==============
    /// 指定工具不存在
    #[error("Tool not found: {0}")]
    ToolNotFound(ToolId),

    /// 工具权限不足
    #[error("Tool permission denied for {0}: requires {1:?}")]
    PermissionDenied(ToolId, crate::types::PermissionLevel),

    /// 工具执行失败
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// 工具参数校验失败
    #[error("Tool validation failed: {0}")]
    ToolValidationFailed(String),

    // ============== 通道错误 ==============
    /// 通道适配器错误
    #[error("Channel error: {0}")]
    ChannelError(#[from] ChannelError),

    // ============== 插件错误 ==============
    /// 插件运行错误
    #[error("Plugin error: {0}")]
    PluginError(#[from] PluginError),

    // ============== 调度错误 ==============
    /// 调度任务不存在
    #[error("Job not found: {0}")]
    JobNotFound(JobId),

    /// 调度任务执行失败
    #[error("Job execution failed: {0}")]
    JobExecutionFailed(String),

    /// 调度规则发生冲突
    #[error("Schedule conflict: {0}")]
    ScheduleConflict(String),

    // ============== 认证错误 ==============
    /// 认证失败
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// 访问令牌已过期
    #[error("Token expired")]
    TokenExpired,

    /// 访问令牌无效
    #[error("Invalid token")]
    InvalidToken,

    /// 设备尚未完成配对
    #[error("Device not paired: {0}")]
    DeviceNotPaired(DeviceId),

    // ============== 协议错误 ==============
    /// 协议处理失败
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// 消息格式无效
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// 协议版本不受支持
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(String),

    // ============== 存储错误 ==============
    /// 存储层错误
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    // ============== 配置错误 ==============
    /// 配置无效
    #[error("Configuration error: {0}")]
    ConfigError(String),

    // ============== 幂等性错误 ==============
    /// 幂等键冲突
    #[error("Idempotency conflict: {0}")]
    IdempotencyConflict(String),

    /// 幂等键已过期
    #[error("Idempotency key expired: {0}")]
    IdempotencyKeyExpired(String),

    // ============== 通用错误 ==============
    /// 序列化或反序列化失败
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// 内部错误
    #[error("Internal error: {0}")]
    InternalError(String),

    /// 功能尚未实现
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl UHorseError {
    /// 获取错误码
    pub fn code(&self) -> ErrorCode {
        match self {
            UHorseError::SessionNotFound(_) => ErrorCode::SessionNotFound,
            UHorseError::SessionExpired(_) => ErrorCode::SessionExpired,
            UHorseError::SessionIsolated(_) => ErrorCode::SessionIsolated,
            UHorseError::ToolNotFound(_) => ErrorCode::ToolNotFound,
            UHorseError::PermissionDenied(_, _) => ErrorCode::ToolPermissionDenied,
            UHorseError::ToolValidationFailed(_) => ErrorCode::ToolValidationFailed,
            UHorseError::ToolExecutionFailed(_) => ErrorCode::ToolExecutionFailed,
            UHorseError::ChannelError(e) => e.code(),
            UHorseError::PluginError(e) => e.code(),
            UHorseError::JobNotFound(_) => ErrorCode::JobNotFound,
            UHorseError::JobExecutionFailed(_) => ErrorCode::JobExecutionFailed,
            UHorseError::ScheduleConflict(_) => ErrorCode::ScheduleConflict,
            UHorseError::AuthFailed(_) => ErrorCode::Unauthorized,
            UHorseError::TokenExpired => ErrorCode::TokenExpired,
            UHorseError::InvalidToken => ErrorCode::InvalidToken,
            UHorseError::DeviceNotPaired(_) => ErrorCode::DeviceNotPaired,
            UHorseError::ProtocolError(_) => ErrorCode::InvalidMessage,
            UHorseError::InvalidMessage(_) => ErrorCode::InvalidMessage,
            UHorseError::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
            UHorseError::StorageError(e) => e.code(),
            UHorseError::IdempotencyConflict(_) => ErrorCode::IdempotencyConflict,
            UHorseError::IdempotencyKeyExpired(_) => ErrorCode::IdempotencyKeyExpired,
            UHorseError::InternalError(_) => ErrorCode::InternalError,
            UHorseError::NotImplemented(_) => ErrorCode::NotImplemented,
            UHorseError::ConfigError(_) => ErrorCode::InternalError,
            UHorseError::SerializationError(_) => ErrorCode::InternalError,
        }
    }

    /// 是否为客户端错误
    pub fn is_client_error(&self) -> bool {
        self.code().is_client_error()
    }

    /// 是否为服务端错误
    pub fn is_server_error(&self) -> bool {
        self.code().is_server_error()
    }
}

/// 通道错误
#[derive(Error, Debug)]
pub enum ChannelError {
    /// 通道发送失败
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// 通道响应校验失败
    #[error("Verification failed")]
    VerificationFailed,

    /// 通道触发限流
    #[error("Rate limited")]
    RateLimited,

    /// 通道配置无效
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// 通道连接已丢失
    #[error("Connection lost")]
    ConnectionLost,

    /// 通道请求超时
    #[error("Timeout")]
    Timeout,

    /// 通道连接失败
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// 通道返回无效响应
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl ChannelError {
    /// 获取通道错误码
    pub fn code(&self) -> ErrorCode {
        match self {
            ChannelError::SendFailed(_) => ErrorCode::InternalError,
            ChannelError::VerificationFailed => ErrorCode::Unauthorized,
            ChannelError::RateLimited => ErrorCode::InternalError,
            ChannelError::ConfigError(_) => ErrorCode::InternalError,
            ChannelError::ConnectionLost => ErrorCode::InternalError,
            ChannelError::Timeout => ErrorCode::InternalError,
            ChannelError::ConnectionError(_) => ErrorCode::InternalError,
            ChannelError::InvalidResponse(_) => ErrorCode::InternalError,
        }
    }
}

/// 插件错误
#[derive(Error, Debug)]
pub enum PluginError {
    /// 插件不存在
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// 插件方法不存在
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// 插件执行超时
    #[error("Execution timeout")]
    Timeout,

    /// 插件进程崩溃
    #[error("Plugin crashed")]
    Crashed,

    /// 插件初始化失败
    #[error("Initialization failed: {0}")]
    InitFailed(String),

    /// 插件返回无效响应
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl PluginError {
    /// 获取插件错误码
    pub fn code(&self) -> ErrorCode {
        match self {
            PluginError::NotFound(_) => ErrorCode::ToolNotFound,
            PluginError::MethodNotFound(_) => ErrorCode::ToolNotFound,
            PluginError::Timeout => ErrorCode::ToolExecutionFailed,
            PluginError::Crashed => ErrorCode::ToolExecutionFailed,
            PluginError::InitFailed(_) => ErrorCode::ToolExecutionFailed,
            PluginError::InvalidResponse(_) => ErrorCode::ToolExecutionFailed,
        }
    }
}

/// 存储错误
#[derive(Error, Debug)]
pub enum StorageError {
    /// 数据库操作失败
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// 数据迁移失败
    #[error("Migration failed: {0}")]
    MigrationError(String),

    /// 存储连接失败
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// 查询执行失败
    #[error("Query error: {0}")]
    QueryError(String),

    /// 事务执行失败
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// 记录不存在
    #[error("Record not found: {0}")]
    NotFound(String),
}

impl StorageError {
    /// 获取存储错误码
    pub fn code(&self) -> ErrorCode {
        match self {
            StorageError::DatabaseError(_) => ErrorCode::InternalError,
            StorageError::MigrationError(_) => ErrorCode::InternalError,
            StorageError::ConnectionError(_) => ErrorCode::InternalError,
            StorageError::QueryError(_) => ErrorCode::InternalError,
            StorageError::TransactionError(_) => ErrorCode::InternalError,
            StorageError::NotFound(_) => ErrorCode::SessionNotFound,
        }
    }
}

/// Result 类型别名
pub type Result<T, E = UHorseError> = std::result::Result<T, E>;
