//! # JSONL 日志记录器
//!
//! JSON Lines 格式的日志记录，用于审计和调试。

use openclaw_core::Result;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::{debug, instrument};

/// JSONL 日志记录器
#[derive(Debug)]
pub struct JsonlLogger {
    writer: BufWriter<tokio::fs::File>,
}

impl JsonlLogger {
    /// 创建新的 JSONL 记录器
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // 确保目录存在
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| openclaw_core::StorageError::ConnectionError(e.to_string()))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|e| openclaw_core::StorageError::ConnectionError(e.to_string()))?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    /// 记录日志条目
    #[instrument(skip(self, entry))]
    pub async fn log(&mut self, entry: &JsonlEntry) -> Result<()> {
        let json = serde_json::to_string(entry)
            .map_err(|e| openclaw_core::StorageError::DatabaseError(e.to_string()))?;

        self.writer.write_all(json.as_bytes()).await
            .map_err(|e| openclaw_core::StorageError::DatabaseError(e.to_string()))?;
        self.writer.write_all(b"\n").await
            .map_err(|e| openclaw_core::StorageError::DatabaseError(e.to_string()))?;
        self.writer.flush().await
            .map_err(|e| openclaw_core::StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 刷新缓冲区
    pub async fn flush(&mut self) -> Result<()> {
        self.writer.flush().await
            .map_err(|e| openclaw_core::StorageError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

/// JSONL 日志条目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonlEntry {
    pub level: LogLevel,
    pub timestamp: u64,
    pub session_id: Option<String>,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::DEBUG => write!(f, "DEBUG"),
            LogLevel::INFO => write!(f, "INFO"),
            LogLevel::WARN => write!(f, "WARN"),
            LogLevel::ERROR => write!(f, "ERROR"),
        }
    }
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => LogLevel::DEBUG,
            tracing::Level::DEBUG => LogLevel::DEBUG,
            tracing::Level::INFO => LogLevel::INFO,
            tracing::Level::WARN => LogLevel::WARN,
            tracing::Level::ERROR => LogLevel::ERROR,
        }
    }
}
