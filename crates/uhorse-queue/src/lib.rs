//! # uHorse Queue
//!
//! 消息队列层，提供 NATS 集成、任务队列、死信队列和重试策略

pub mod dead_letter;
pub mod nats;
pub mod retry;
pub mod task_queue;

pub use dead_letter::DeadLetterQueue;
pub use nats::NatsClient;
pub use retry::{RetryPolicy, RetryStrategy};
pub use task_queue::{Task, TaskQueue};

/// Queue result type
pub type Result<T> = anyhow::Result<T>;
