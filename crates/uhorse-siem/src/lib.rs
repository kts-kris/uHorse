//! # uHorse SIEM Module
//!
//! SIEM (Security Information and Event Management) 集成模块
//!
//! ## Features
//!
//! - 审计日志导出 (CEF/JSON)
//! - Splunk HEC 集成
//! - Datadog Logs API 集成
//! - 安全告警

pub mod alerts;
pub mod datadog;
pub mod export;
pub mod splunk;

pub use alerts::{
    default_alert_rules, Alert, AlertCondition, AlertManager, AlertRule, AlertSeverity, AlertStatus,
};
pub use datadog::{DatadogClient, DatadogConfig, DatadogLogEntry};
pub use export::{AuditEvent, AuditExporter, ExportFormat};
pub use splunk::{SplunkClient, SplunkConfig, SplunkEvent};

use thiserror::Error;

/// SIEM 错误类型
#[derive(Error, Debug)]
pub enum SiemError {
    #[error("Export error: {0}")]
    ExportError(String),

    #[error("Splunk error: {0}")]
    SplunkError(String),

    #[error("Datadog error: {0}")]
    DatadogError(String),

    #[error("Alert error: {0}")]
    AlertError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// SIEM 结果类型
pub type Result<T> = std::result::Result<T, SiemError>;

/// 导出模块重导出
pub mod export_for_splunk {
    pub use crate::export::AuditEvent;
}
