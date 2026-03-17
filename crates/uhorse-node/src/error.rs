//! 错误定义

use thiserror::Error;

/// 节点错误
#[derive(Debug, Error)]
pub enum NodeError {
    /// 工作空间错误
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    Permission(String),

    /// 执行错误
    #[error("Execution error: {0}")]
    Execution(String),

    /// 连接错误
    #[error("Connection error: {0}")]
    Connection(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// 协议错误
    #[error("Protocol error: {0}")]
    Protocol(#[from] uhorse_protocol::ProtocolError),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 超时
    #[error("Timeout: {0}")]
    Timeout(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    Internal(String),
}

/// 节点结果类型
pub type NodeResult<T> = std::result::Result<T, NodeError>;
