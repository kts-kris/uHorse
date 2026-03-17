//! Hub 错误定义
//!
//! 定义 Hub 模块的错误类型

use thiserror::Error;
use uhorse_protocol::NodeId;

/// Hub 错误
#[derive(Debug, Error)]
pub enum HubError {
    /// 节点未找到
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// 节点数量达到上限
    #[error("Maximum number of nodes reached")]
    NodeLimitReached,

    /// 节点错误
    #[error("Node error: {0}")]
    Node(String),

    /// 任务错误
    #[error("Task error: {0}")]
    Task(String),

    /// 调度错误
    #[error("Schedule error: {0}")]
    Schedule(String),

    /// 通信错误
    #[error("Communication error: {0}")]
    Communication(String),

    /// 超时错误
    #[error("Timeout: {0}")]
    Timeout(String),

    /// 权限错误
    #[error("Permission denied: {0}")]
    Permission(String),

    /// 配置错误
    #[error("Config error: {0}")]
    Config(String),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 内部错误
    #[error("Internal error: {0}")]
    Internal(String),

    /// 未实现
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Hub 结果类型
pub type HubResult<T> = std::result::Result<T, HubError>;
