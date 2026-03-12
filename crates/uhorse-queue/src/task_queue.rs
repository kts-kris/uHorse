//! Task queue implementation

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::nats::NatsClient;
use super::retry::RetryPolicy;

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task is being retried
    Retrying,
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority (processed first)
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task<T> {
    /// Unique task ID
    pub id: String,
    /// Task type/name
    pub task_type: String,
    /// Task payload
    pub payload: T,
    /// Priority
    pub priority: TaskPriority,
    /// Current status
    pub status: TaskStatus,
    /// Number of retries attempted
    pub retry_count: u32,
    /// Maximum retries allowed
    pub max_retries: u32,
    /// When the task was created (timestamp in ms)
    pub created_at: u64,
    /// When the task was last updated (timestamp in ms)
    pub updated_at: u64,
    /// Scheduled execution time (for delayed tasks, timestamp in ms)
    pub scheduled_at: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
    /// Task metadata
    pub metadata: serde_json::Value,
}

impl<T: Clone + Serialize + DeserializeOwned> Task<T> {
    /// Create a new task
    pub fn new(id: impl Into<String>, task_type: impl Into<String>, payload: T) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            id: id.into(),
            task_type: task_type.into(),
            payload,
            priority: TaskPriority::default(),
            status: TaskStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            created_at: now,
            updated_at: now,
            scheduled_at: None,
            error: None,
            metadata: serde_json::json!({}),
        }
    }

    /// Set task priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Schedule for future execution
    pub fn with_schedule(mut self, delay: Duration) -> Self {
        self.scheduled_at = Some(
            chrono::Utc::now().timestamp_millis() as u64 + delay.as_millis() as u64
        );
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Mark task as started
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// Mark task as completed
    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// Mark task as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = if self.can_retry() {
            TaskStatus::Retrying
        } else {
            TaskStatus::Failed
        };
        self.error = Some(error.into());
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.status = TaskStatus::Pending;
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// Serialize to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| anyhow!("Failed to serialize task: {}", e))
    }

    /// Deserialize from JSON bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self>
    where
        T: DeserializeOwned,
    {
        serde_json::from_slice(data).map_err(|e| anyhow!("Failed to deserialize task: {}", e))
    }
}

/// Task handler trait
#[async_trait::async_trait]
pub trait TaskHandler<T>: Send + Sync {
    /// Handle a task
    async fn handle(&self, task: Task<T>) -> Result<()>;
}

/// Task queue configuration
#[derive(Debug, Clone)]
pub struct TaskQueueConfig {
    /// NATS subject for task queue
    pub subject: String,
    /// Default retry policy
    pub retry_policy: RetryPolicy,
    /// Maximum concurrent tasks
    pub max_concurrent: usize,
    /// Task timeout
    pub task_timeout: Duration,
}

impl Default for TaskQueueConfig {
    fn default() -> Self {
        Self {
            subject: "uhorse.tasks".to_string(),
            retry_policy: RetryPolicy::default(),
            max_concurrent: 10,
            task_timeout: Duration::from_secs(300),
        }
    }
}

/// Task queue for async task processing
pub struct TaskQueue<T> {
    /// NATS client
    nats: Arc<NatsClient>,
    /// Configuration
    config: TaskQueueConfig,
    /// Task handlers by type
    handlers: Arc<RwLock<std::collections::HashMap<String, Box<dyn TaskHandler<T>>>>>,
    /// Pending tasks (in-memory)
    pending: Arc<RwLock<VecDeque<Task<T>>>>,
}

impl<T: Clone + Send + Sync + Serialize + DeserializeOwned + 'static> TaskQueue<T> {
    /// Create a new task queue
    pub fn new(nats: Arc<NatsClient>, config: TaskQueueConfig) -> Self {
        Self {
            nats,
            config,
            handlers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            pending: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Register a task handler
    pub async fn register_handler(&self, task_type: &str, handler: Box<dyn TaskHandler<T>>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(task_type.to_string(), handler);
        info!("Registered handler for task type: {}", task_type);
    }

    /// Enqueue a task
    pub async fn enqueue(&self, task: Task<T>) -> Result<()> {
        // Serialize task
        let data = task.to_bytes()?;

        // Publish to NATS
        let subject = format!("{}.{}", self.config.subject, task.task_type);
        self.nats.publish(&subject, data).await?;

        // Add to pending queue
        {
            let mut pending = self.pending.write().await;
            pending.push_back(task.clone());
        }

        debug!("Enqueued task {} of type {}", task.id, task.task_type);
        Ok(())
    }

    /// Process pending tasks
    pub async fn process_pending(&self) -> Result<usize> {
        let mut processed = 0;
        let mut to_remove = Vec::new();
        let now = chrono::Utc::now().timestamp_millis() as u64;

        {
            let mut pending = self.pending.write().await;
            let mut i = 0;
            while i < pending.len() {
                let task = &mut pending[i];

                // Skip non-pending tasks
                if task.status != TaskStatus::Pending {
                    i += 1;
                    continue;
                }

                // Check scheduled time
                if let Some(scheduled_at) = task.scheduled_at {
                    if now < scheduled_at {
                        i += 1;
                        continue;
                    }
                }

                // Get handler and process within the read lock scope
                {
                    let handlers = self.handlers.read().await;
                    let handler = match handlers.get(&task.task_type) {
                        Some(h) => h.as_ref(),
                        None => {
                            warn!("No handler for task type: {}", task.task_type);
                            to_remove.push(i);
                            i += 1;
                            continue;
                        }
                    };

                    // Start task
                    task.start();
                    let task_clone = task.clone();

                    // Handle task
                    match handler.handle(task_clone).await {
                        Ok(()) => {
                            task.complete();
                            to_remove.push(i);
                            processed += 1;
                            debug!("Task {} completed successfully", task.id);
                        }
                        Err(e) => {
                            task.fail(e.to_string());
                            if task.can_retry() {
                                task.increment_retry();
                                info!("Task {} will be retried (attempt {}/{})",
                                     task.id, task.retry_count, task.max_retries);
                            } else {
                                error!("Task {} failed permanently: {}", task.id, e);
                                to_remove.push(i);
                            }
                        }
                    }
                }

                i += 1;
            }

            // Remove completed/failed tasks (in reverse order to preserve indices)
            for i in to_remove.into_iter().rev() {
                pending.remove(i);
            }
        }

        Ok(processed)
    }

    /// Get queue statistics
    pub async fn stats(&self) -> TaskQueueStats {
        let pending = self.pending.read().await;
        let pending_count = pending.len();
        let pending_by_type: std::collections::HashMap<String, usize> = pending
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .fold(std::collections::HashMap::new(), |mut acc, t| {
                *acc.entry(t.task_type.clone()).or_insert(0) += 1;
                acc
            });

        TaskQueueStats {
            pending_count,
            pending_by_type,
            max_concurrent: self.config.max_concurrent,
        }
    }
}

/// Task queue statistics
#[derive(Debug, Clone)]
pub struct TaskQueueStats {
    /// Total pending tasks
    pub pending_count: usize,
    /// Pending tasks by type
    pub pending_by_type: std::collections::HashMap<String, usize>,
    /// Maximum concurrent tasks
    pub max_concurrent: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task: Task<String> = Task::new("task-1", "test", "payload".to_string());
        assert_eq!(task.id, "task-1");
        assert_eq!(task.task_type, "test");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_priority() {
        let task: Task<String> = Task::new("task-1", "test", "payload".to_string())
            .with_priority(TaskPriority::High);
        assert_eq!(task.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_retry() {
        let mut task: Task<String> = Task::new("task-1", "test", "payload".to_string())
            .with_max_retries(2);
        assert!(task.can_retry());

        // First failure -> Retrying
        task.fail("test error");
        assert_eq!(task.status, TaskStatus::Retrying);

        // Increment retry count (retry_count = 1)
        task.increment_retry();
        assert!(task.can_retry());

        // Second failure -> Retrying (retry_count = 1 < max_retries = 2)
        task.fail("test error 2");
        assert_eq!(task.status, TaskStatus::Retrying);

        // Increment retry count (retry_count = 2)
        task.increment_retry();
        assert!(!task.can_retry());

        // Third failure -> Failed (retry_count = 2 >= max_retries = 2)
        task.fail("test error 3");
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[test]
    fn test_task_serialization() {
        let task: Task<String> = Task::new("task-1", "test", "payload".to_string());
        let bytes = task.to_bytes().unwrap();
        let decoded: Task<String> = Task::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.id, task.id);
        assert_eq!(decoded.task_type, task.task_type);
        assert_eq!(decoded.payload, task.payload);
    }
}
