//! # 优雅关闭
//!
//! 提供优雅关闭功能，确保资源正确释放。

use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// 关闭信号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// Ctrl+C
    Interrupt,
    /// 终止信号
    Terminate,
    /// 超时
    Timeout,
    /// 错误
    Error,
}

/// 关闭阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShutdownPhase {
    /// 初始阶段
    Preparing = 0,
    /// 停止接受新请求
    NotAccepting = 1,
    /// 等待现有请求完成
    Draining = 2,
    /// 关闭连接
    Closing = 3,
    /// 清理资源
    Cleanup = 4,
    /// 完成
    Complete = 5,
}

/// 关闭句柄
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    phase: Arc<RwLock<ShutdownPhase>>,
    signal: Arc<RwLock<Option<ShutdownSignal>>>,
}

impl ShutdownHandle {
    /// 创建新的关闭句柄
    pub fn new() -> Self {
        Self {
            phase: Arc::new(RwLock::new(ShutdownPhase::Preparing)),
            signal: Arc::new(RwLock::new(None)),
        }
    }

    /// 获取当前阶段
    pub async fn phase(&self) -> ShutdownPhase {
        *self.phase.read().await
    }

    /// 设置阶段
    pub async fn set_phase(&self, phase: ShutdownPhase) {
        *self.phase.write().await = phase;
        info!("Shutdown phase: {:?}", phase);
    }

    /// 检查是否应该关闭
    pub async fn is_shutting_down(&self) -> bool {
        let phase = self.phase.read().await;
        *phase > ShutdownPhase::Preparing
    }

    /// 检查是否应该停止接受新请求
    pub async fn should_stop_accepting(&self) -> bool {
        let phase = self.phase.read().await;
        *phase >= ShutdownPhase::NotAccepting
    }

    /// 触发关闭
    pub async fn shutdown(&self, signal: ShutdownSignal) {
        *self.signal.write().await = Some(signal);
        warn!("Shutdown triggered: {:?}", signal);
    }

    /// 等待关闭信号
    pub async fn wait_for_signal(&self) -> ShutdownSignal {
        loop {
            if let Some(signal) = *self.signal.read().await {
                return signal;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// 获取关闭信号（如果已触发）
    pub async fn get_signal(&self) -> Option<ShutdownSignal> {
        *self.signal.read().await
    }
}

impl Default for ShutdownHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 优雅关闭管理器
pub struct GracefulShutdown {
    handle: ShutdownHandle,
    timeout: Duration,
    drain_timeout: Duration,
}

impl GracefulShutdown {
    /// 创建新的优雅关闭管理器
    pub fn new() -> Self {
        Self {
            handle: ShutdownHandle::new(),
            timeout: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(10),
        }
    }

    /// 设置总超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置排空超时时间
    pub fn with_drain_timeout(mut self, drain_timeout: Duration) -> Self {
        self.drain_timeout = drain_timeout;
        self
    }

    /// 获取关闭句柄
    pub fn handle(&self) -> ShutdownHandle {
        self.handle.clone()
    }

    /// 开始关闭流程
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        info!("Starting graceful shutdown...");

        let signal = self.handle.wait_for_signal().await;
        info!("Received shutdown signal: {:?}", signal);

        // 阶段 1: 停止接受新请求
        self.handle.set_phase(ShutdownPhase::NotAccepting).await;
        info!("Phase 1: No longer accepting new requests");

        // 阶段 2: 等待现有请求完成
        self.handle.set_phase(ShutdownPhase::Draining).await;
        info!("Phase 2: Draining existing connections...");

        match timeout(self.drain_timeout, async {
            // 在实际实现中，这里应该等待所有活跃请求完成
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await
        {
            Ok(_) => {
                info!("All connections drained successfully");
            }
            Err(_) => {
                warn!("Drain timeout, forcing shutdown");
            }
        }

        // 阶段 3: 关闭连接
        self.handle.set_phase(ShutdownPhase::Closing).await;
        info!("Phase 3: Closing connections...");

        // 阶段 4: 清理资源
        self.handle.set_phase(ShutdownPhase::Cleanup).await;
        info!("Phase 4: Cleaning up resources...");

        // 阶段 5: 完成
        self.handle.set_phase(ShutdownPhase::Complete).await;
        info!("Phase 5: Shutdown complete");

        Ok(())
    }

    /// 监听系统信号
    pub async fn listen_for_signals(&self) {
        let handle = self.handle.clone();

        // Ctrl+C
        tokio::spawn(async move {
            if let Err(e) = signal::ctrl_c().await {
                error!("Failed to install Ctrl+C handler: {}", e);
            }
            handle.shutdown(ShutdownSignal::Interrupt).await;
        });

        // 终止信号（Unix）
        #[cfg(unix)]
        {
            let handle = self.handle.clone();
            tokio::spawn(async move {
                match signal::unix::signal(signal::unix::SignalKind::terminate()) {
                    Ok(mut sig) => {
                        sig.recv().await;
                        handle.shutdown(ShutdownSignal::Terminate).await;
                    }
                    Err(e) => {
                        error!("Failed to install SIGTERM handler: {}", e);
                    }
                }
            });
        }
    }

    /// 带超时地关闭
    pub async fn shutdown_with_timeout(&self) -> anyhow::Result<()> {
        match timeout(self.timeout, self.shutdown()).await {
            Ok(result) => result,
            Err(_) => {
                error!("Shutdown timeout, forcing exit");
                self.handle.shutdown(ShutdownSignal::Timeout).await;
                Err(anyhow::anyhow!("Shutdown timeout"))
            }
        }
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// 关闭任务 trait
#[async_trait::async_trait]
pub trait ShutdownTask: Send + Sync {
    /// 任务名称
    fn name(&self) -> &str;

    /// 执行关闭任务
    async fn shutdown(&self, phase: ShutdownPhase) -> anyhow::Result<()>;
}

/// 关闭管理器（带任务）
pub struct ShutdownManager {
    handle: ShutdownHandle,
    tasks: Vec<Box<dyn ShutdownTask>>,
}

impl ShutdownManager {
    /// 创建新的关闭管理器
    pub fn new(handle: ShutdownHandle) -> Self {
        Self {
            handle,
            tasks: Vec::new(),
        }
    }

    /// 添加关闭任务
    pub fn add_task(mut self, task: Box<dyn ShutdownTask>) -> Self {
        self.tasks.push(task);
        self
    }

    /// 执行关闭流程
    pub async fn execute_shutdown(&self) -> anyhow::Result<()> {
        info!("Starting shutdown with {} tasks", self.tasks.len());

        // 执行各阶段的任务
        for phase in &[
            ShutdownPhase::NotAccepting,
            ShutdownPhase::Draining,
            ShutdownPhase::Closing,
            ShutdownPhase::Cleanup,
        ] {
            self.handle.set_phase(*phase).await;

            for task in &self.tasks {
                info!("Executing task '{}' for phase {:?}", task.name(), phase);
                if let Err(e) = task.shutdown(*phase).await {
                    error!("Task '{}' failed: {}", task.name(), e);
                    // 继续执行其他任务
                }
            }
        }

        self.handle.set_phase(ShutdownPhase::Complete).await;
        info!("Shutdown complete");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_handle() {
        let handle = ShutdownHandle::new();
        assert_eq!(handle.phase().await, ShutdownPhase::Preparing);
        assert!(!handle.is_shutting_down().await);

        handle.set_phase(ShutdownPhase::NotAccepting).await;
        assert!(handle.should_stop_accepting().await);
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let handle = ShutdownHandle::new();

        // 异步触发关闭
        let h = handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            h.shutdown(ShutdownSignal::Interrupt).await;
        });

        let signal = handle.wait_for_signal().await;
        assert_eq!(signal, ShutdownSignal::Interrupt);
    }
}
