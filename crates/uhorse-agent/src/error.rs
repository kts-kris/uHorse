//! # Agent 框架错误类型

use thiserror::Error;

/// Agent 框架错误
#[derive(Error, Debug)]
pub enum AgentError {
    /// Gateway 错误
    #[error("Gateway error: {0}")]
    Gateway(String),

    /// Agent 错误
    #[error("Agent error: {0}")]
    Agent(String),

    /// Skill 错误
    #[error("Skill error: {0}")]
    Skill(String),

    /// Memory 错误
    #[error("Memory error: {0}")]
    Memory(String),

    /// 路由错误
    #[error("Router error: {0}")]
    Router(String),

    /// LLM 调用失败
    #[error("LLM error: {0}")]
    LLM(String),

    /// 技能执行失败
    #[error("Skill execution failed: {skill}: {error}")]
    SkillExecution { skill: String, error: String },

    /// 会话未找到
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// 无效配置
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 其他错误
    #[error("Other: {0}")]
    Other(String),
}

/// Agent 框架结果类型
pub type AgentResult<T> = Result<T, AgentError>;
