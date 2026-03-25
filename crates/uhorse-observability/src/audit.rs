//! # 审计日志
//!
//! 记录系统操作的审计日志。

use serde::{Deserialize, Serialize};
use uhorse_core::Result;

/// 审计日志记录器
#[derive(Debug)]
pub struct AuditLogger {
    // TODO: 添加存储后端
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        tracing::info!("Audit: {:?}", event);
        Ok(())
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// 审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub level: AuditLevel,
    pub category: AuditCategory,
    pub actor: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub details: Option<serde_json::Value>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditCategory {
    Auth,
    Tool,
    Scheduler,
    Session,
}
