//! Dead letter queue for failed tasks

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::nats::NatsClient;
use super::task_queue::Task;

/// Dead letter entry
#[derive(Debug, Clone)]
pub struct DeadLetterEntry<T> {
    /// Original task
    pub task: Task<T>,
    /// Reason for dead letter
    pub reason: String,
    /// When added to dead letter queue (timestamp in milliseconds)
    pub dead_lettered_at: u64,
    /// Original error message
    pub original_error: String,
    /// Number of retry attempts before dead letter
    pub retry_attempts: u32,
}

impl<T: Clone + Serialize + DeserializeOwned> DeadLetterEntry<T> {
    /// Create a new dead letter entry
    pub fn new(task: Task<T>, reason: impl Into<String>) -> Self {
        let original_error = task.error.clone().unwrap_or_default();
        let retry_attempts = task.retry_count;
        Self {
            task,
            reason: reason.into(),
            dead_lettered_at: chrono::Utc::now().timestamp_millis() as u64,
            original_error,
            retry_attempts,
        }
    }
}

/// Dead letter queue configuration
#[derive(Debug, Clone)]
pub struct DeadLetterConfig {
    /// NATS subject for dead letter queue
    pub subject: String,
    /// Maximum entries to keep
    pub max_entries: usize,
    /// Retention period for dead letter entries
    pub retention: Duration,
}

impl Default for DeadLetterConfig {
    fn default() -> Self {
        Self {
            subject: "uhorse.dead_letter".to_string(),
            max_entries: 10000,
            retention: Duration::from_secs(86400), // 24 hours
        }
    }
}

/// Dead letter queue for failed tasks
pub struct DeadLetterQueue<T> {
    /// NATS client
    nats: Arc<NatsClient>,
    /// Configuration
    config: DeadLetterConfig,
    /// In-memory storage
    entries: Arc<RwLock<Vec<DeadLetterEntry<T>>>>,
}

impl<T: Clone + Send + Sync + Serialize + DeserializeOwned + 'static> DeadLetterQueue<T> {
    /// Create a new dead letter queue
    pub fn new(nats: Arc<NatsClient>, config: DeadLetterConfig) -> Self {
        Self {
            nats,
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a task to the dead letter queue
    pub async fn add(&self, task: Task<T>, reason: impl Into<String>) -> Result<()> {
        let entry = DeadLetterEntry::new(task, reason);

        // Store in memory
        {
            let mut entries = self.entries.write().await;

            // Enforce max entries limit
            while entries.len() >= self.config.max_entries {
                entries.remove(0);
                warn!("Removed oldest dead letter entry to make room");
            }

            entries.push(entry);
        }

        info!("Added task to dead letter queue");
        Ok(())
    }

    /// Get all dead letter entries
    pub async fn get_all(&self) -> Vec<DeadLetterEntry<T>> {
        let entries = self.entries.read().await;
        entries.clone()
    }

    /// Get entries by task type
    pub async fn get_by_type(&self, task_type: &str) -> Vec<DeadLetterEntry<T>> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| e.task.task_type == task_type)
            .cloned()
            .collect()
    }

    /// Get entry by task ID
    pub async fn get_by_id(&self, task_id: &str) -> Option<DeadLetterEntry<T>> {
        let entries = self.entries.read().await;
        entries.iter().find(|e| e.task.id == task_id).cloned()
    }

    /// Remove an entry by task ID
    pub async fn remove(&self, task_id: &str) -> Result<bool> {
        let mut entries = self.entries.write().await;
        if let Some(pos) = entries.iter().position(|e| e.task.id == task_id) {
            entries.remove(pos);
            debug!("Removed dead letter entry for task {}", task_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Requeue a task for retry
    pub async fn requeue(&self, task_id: &str) -> Result<Option<Task<T>>> {
        let mut entries = self.entries.write().await;
        if let Some(pos) = entries.iter().position(|e| e.task.id == task_id) {
            let entry = entries.remove(pos);
            let mut task = entry.task;
            task.status = super::task_queue::TaskStatus::Pending;
            task.retry_count = 0;
            task.error = None;
            info!("Requeued task {} from dead letter queue", task_id);
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    /// Get queue statistics
    pub async fn stats(&self) -> DeadLetterStats {
        let entries = self.entries.read().await;
        let total_count = entries.len();
        let mut by_type: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for e in entries.iter() {
            *by_type.entry(e.task.task_type.clone()).or_insert(0) += 1;
        }

        DeadLetterStats {
            total_count,
            by_type,
            max_entries: self.config.max_entries,
        }
    }

    /// Clear all entries
    pub async fn clear(&self) -> Result<u64> {
        let mut entries = self.entries.write().await;
        let count = entries.len() as u64;
        entries.clear();
        warn!("Cleared {} dead letter entries", count);
        Ok(count)
    }

    /// Purge expired entries
    pub async fn purge_expired(&self) -> Result<u64> {
        let mut entries = self.entries.write().await;
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let retention_ms = self.config.retention.as_millis() as u64;
        let initial_len = entries.len();

        entries.retain(|e| {
            now.saturating_sub(e.dead_lettered_at) < retention_ms
        });

        let purged = initial_len - entries.len();
        if purged > 0 {
            info!("Purged {} expired dead letter entries", purged);
        }
        Ok(purged as u64)
    }
}

/// Dead letter queue statistics
#[derive(Debug, Clone)]
pub struct DeadLetterStats {
    /// Total entries
    pub total_count: usize,
    /// Entries by task type
    pub by_type: std::collections::HashMap<String, usize>,
    /// Maximum entries allowed
    pub max_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dead_letter_entry() {
        let task: Task<String> = Task::new("task-1", "test", "payload".to_string());
        let entry = DeadLetterEntry::new(task, "Max retries exceeded");

        assert_eq!(entry.task.id, "task-1");
        assert_eq!(entry.reason, "Max retries exceeded");
    }
}
