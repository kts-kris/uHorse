//! # uHorse Webhook Module
//!
//! Webhook 增强模块
//!
//! ## Features
//!
//! - 重试机制 (指数退避)
//! - 签名验证 (HMAC-SHA256)
//! - 模板系统
//! - 历史查询

pub mod retry;
pub mod signature;
pub mod template;
pub mod history;
pub mod client;

pub use retry::{RetryPolicy, RetryState, RetryableError};
pub use signature::{SignatureVerifier, SigningConfig};
pub use template::{WebhookTemplate, TemplateEngine};
pub use history::{WebhookHistory, WebhookRecord, WebhookStatus};
pub use client::{WebhookClient, WebhookConfig, WebhookEvent};

use thiserror::Error;

/// Webhook 错误类型
#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Retry exhausted: {0}")]
    RetryExhausted(String),

    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    #[error("Template error: {0}")]
    TemplateError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Timeout")]
    Timeout,
}

/// Webhook 结果类型
pub type Result<T> = std::result::Result<T, WebhookError>;
