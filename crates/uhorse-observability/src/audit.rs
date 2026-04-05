//! # 审计日志
//!
//! 记录系统操作的审计日志。

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uhorse_core::{Result, UHorseError};

use crate::audit_persistent::{AuditPersistor, InMemoryAuditStorage};

static GLOBAL_AUDIT_LOGGER: OnceLock<AuditLogger> = OnceLock::new();

/// 审计日志记录器
#[derive(Clone)]
pub struct AuditLogger {
    persistor: Option<Arc<AuditPersistor<InMemoryAuditStorage>>>,
    recorded_events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl std::fmt::Debug for AuditLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditLogger")
            .field("persistor", &self.persistor.is_some())
            .finish()
    }
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            persistor: None,
            recorded_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_in_memory_storage(max_events: usize) -> Self {
        let storage = Arc::new(InMemoryAuditStorage::new(max_events));
        let persistor = Arc::new(AuditPersistor::new(storage));
        Self {
            persistor: Some(persistor),
            recorded_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn install_global(self) -> &'static AuditLogger {
        GLOBAL_AUDIT_LOGGER.get_or_init(|| self)
    }

    pub fn global() -> Option<&'static AuditLogger> {
        GLOBAL_AUDIT_LOGGER.get()
    }

    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        tracing::info!(action = %event.action, category = ?event.category, "Audit event recorded");
        self.recorded_events.write().await.push(event.clone());
        if let Some(persistor) = &self.persistor {
            persistor
                .persist(event)
                .await
                .map_err(|error| UHorseError::InternalError(error.to_string()))?;
        }
        Ok(())
    }

    pub async fn recorded_events(&self) -> Vec<AuditEvent> {
        self.recorded_events.read().await.clone()
    }

    pub async fn clear_recorded_events(&self) {
        self.recorded_events.write().await.clear();
    }
}

pub async fn log_audit_event(event: AuditEvent) -> Result<()> {
    if let Some(logger) = AuditLogger::global() {
        logger.log(event).await
    } else {
        tracing::debug!(action = %event.action, "Audit logger not installed; skipping event persistence");
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
