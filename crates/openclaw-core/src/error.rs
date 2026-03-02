//! # 统一错误定义
//!
//! 定义系统所有错误类型。

use crate::types::{SessionId, ToolId, DeviceId, ErrorCode, JobId};
use thiserror::Error;

/// OpenClaw 统一错误类型
#[derive(Error, Debug)]
pub enum OpenClawError {
    // ============== 会话错误 ==============
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Session expired: {0}")]
    SessionExpired(SessionId),

    #[error("Session isolated: {0}")]
    SessionIsolated(SessionId),

    // ============== 工具错误 ==============
    #[error("Tool not found: {0}")]
    ToolNotFound(ToolId),

    #[error("Tool permission denied for {0}: requires {1:?}")]
    PermissionDenied(ToolId, crate::types::PermissionLevel),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Tool validation failed: {0}")]
    ToolValidationFailed(String),

    // ============== 通道错误 ==============
    #[error("Channel error: {0}")]
    ChannelError(#[from] ChannelError),

    // ============== 插件错误 ==============
    #[error("Plugin error: {0}")]
    PluginError(#[from] PluginError),

    // ============== 调度错误 ==============
    #[error("Job not found: {0}")]
    JobNotFound(JobId),

    #[error("Job execution failed: {0}")]
    JobExecutionFailed(String),

    #[error("Schedule conflict: {0}")]
    ScheduleConflict(String),

    // ============== 认证错误 ==============
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Device not paired: {0}")]
    DeviceNotPaired(DeviceId),

    // ============== 协议错误 ==============
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(String),

    // ============== 存储错误 ==============
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    // ============== 配置错误 ==============
    #[error("Configuration error: {0}")]
    ConfigError(String),

    // ============== 幂等性错误 ==============
    #[error("Idempotency conflict: {0}")]
    IdempotencyConflict(String),

    #[error("Idempotency key expired: {0}")]
    IdempotencyKeyExpired(String),

    // ============== 通用错误 ==============
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl OpenClawError {
    /// 获取错误码
    pub fn code(&self) -> ErrorCode {
        match self {
            OpenClawError::SessionNotFound(_) => ErrorCode::SessionNotFound,
            OpenClawError::SessionExpired(_) => ErrorCode::SessionExpired,
            OpenClawError::SessionIsolated(_) => ErrorCode::SessionIsolated,
            OpenClawError::ToolNotFound(_) => ErrorCode::ToolNotFound,
            OpenClawError::PermissionDenied(_, _) => ErrorCode::ToolPermissionDenied,
            OpenClawError::ToolValidationFailed(_) => ErrorCode::ToolValidationFailed,
            OpenClawError::ToolExecutionFailed(_) => ErrorCode::ToolExecutionFailed,
            OpenClawError::ChannelError(e) => e.code(),
            OpenClawError::PluginError(e) => e.code(),
            OpenClawError::JobNotFound(_) => ErrorCode::JobNotFound,
            OpenClawError::JobExecutionFailed(_) => ErrorCode::JobExecutionFailed,
            OpenClawError::ScheduleConflict(_) => ErrorCode::ScheduleConflict,
            OpenClawError::AuthFailed(_) => ErrorCode::Unauthorized,
            OpenClawError::TokenExpired => ErrorCode::TokenExpired,
            OpenClawError::InvalidToken => ErrorCode::InvalidToken,
            OpenClawError::DeviceNotPaired(_) => ErrorCode::DeviceNotPaired,
            OpenClawError::ProtocolError(_) => ErrorCode::InvalidMessage,
            OpenClawError::InvalidMessage(_) => ErrorCode::InvalidMessage,
            OpenClawError::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
            OpenClawError::StorageError(e) => e.code(),
            OpenClawError::IdempotencyConflict(_) => ErrorCode::IdempotencyConflict,
            OpenClawError::IdempotencyKeyExpired(_) => ErrorCode::IdempotencyKeyExpired,
            OpenClawError::InternalError(_) => ErrorCode::InternalError,
            OpenClawError::NotImplemented(_) => ErrorCode::NotImplemented,
            OpenClawError::ConfigError(_) => ErrorCode::InternalError,
            OpenClawError::SerializationError(_) => ErrorCode::InternalError,
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
    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Verification failed")]
    VerificationFailed,

    #[error("Rate limited")]
    RateLimited,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Connection lost")]
    ConnectionLost,

    #[error("Timeout")]
    Timeout,
}

impl ChannelError {
    pub fn code(&self) -> ErrorCode {
        match self {
            ChannelError::SendFailed(_) => ErrorCode::InternalError,
            ChannelError::VerificationFailed => ErrorCode::Unauthorized,
            ChannelError::RateLimited => ErrorCode::InternalError,
            ChannelError::ConfigError(_) => ErrorCode::InternalError,
            ChannelError::ConnectionLost => ErrorCode::InternalError,
            ChannelError::Timeout => ErrorCode::InternalError,
        }
    }
}


/// 插件错误
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Execution timeout")]
    Timeout,

    #[error("Plugin crashed")]
    Crashed,

    #[error("Initialization failed: {0}")]
    InitFailed(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl PluginError {
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
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Migration failed: {0}")]
    MigrationError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Record not found: {0}")]
    NotFound(String),
}

impl StorageError {
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
pub type Result<T, E = OpenClawError> = std::result::Result<T, E>;

