//! # 多模态错误处理

use thiserror::Error;

/// 多模态错误类型
#[derive(Debug, Error)]
pub enum MultimodalError {
    /// HTTP 请求错误
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// JSON 解析错误
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// API 错误
    #[error("API error: {0}")]
    ApiError(String),

    /// 不支持的格式
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// 文件过大
    #[error("File too large: {0} bytes, max: {1}")]
    FileTooLarge(u64, u64),

    /// 编码错误
    #[error("Encoding error: {0}")]
    EncodingError(String),

    /// 文档解析错误
    #[error("Document parsing error: {0}")]
    ParseError(String),

    /// API Key 未配置
    #[error("API key not configured")]
    ApiKeyNotConfigured,

    /// 超时
    #[error("Request timeout")]
    Timeout,
}

/// 多模态结果类型
pub type Result<T> = std::result::Result<T, MultimodalError>;
