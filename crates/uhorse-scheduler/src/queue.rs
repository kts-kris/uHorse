//! # 执行队列
//!
//! 管理任务执行队列。

use uhorse_core::Result;

/// 执行队列
#[derive(Debug)]
pub struct ExecutionQueue;

impl ExecutionQueue {
    pub fn new() -> Self {
        Self
    }

    pub async fn push(&self, job_id: uhorse_core::JobId) -> Result<()> {
        tracing::info!("Pushing job {} to queue", job_id);
        Ok(())
    }

    pub async fn pop(&self) -> Result<Option<uhorse_core::JobId>> {
        Ok(None)
    }
}

impl Default for ExecutionQueue {
    fn default() -> Self {
        Self::new()
    }
}
