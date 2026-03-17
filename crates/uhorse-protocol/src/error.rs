//! 错误定义

use thiserror::Error;

/// 协议错误
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 无效消息格式
    #[error("Invalid message format: {0}")]
    InvalidMessageFormat(String),

    /// 无效命令
    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    /// 权限不足
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// 任务未找到
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// 节点未找到
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// 连接错误
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// 超时
    #[error("Timeout: {0}")]
    Timeout(String),

    /// 执行错误
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// 验证错误
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// 协议结果类型
pub type ProtocolResult<T> = std::result::Result<T, ProtocolError>;
