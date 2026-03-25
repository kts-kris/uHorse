//! Hub 主模块
//!
//! 云端中枢的核心实现，整合 3.x 模块和 4.0 新增能力

use crate::error::{HubError, HubResult};
use crate::message_router::MessageRouter;
use crate::node_manager::{NodeManager, NodeManagerStats};
use crate::security_integration::SecurityManager;
use crate::task_scheduler::{CompletedTask, SchedulerStats, TaskScheduler, TaskStatusInfo};
use uhorse_channel::DingTalkChannel;
use uhorse_config::DingTalkNotificationBinding;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};
use uhorse_protocol::{
    Command, NodeCapabilities, NodeId, NodeToHub, Priority, TaskContext, TaskId, WorkspaceInfo,
};

/// Hub 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    /// Hub ID
    pub hub_id: String,
    /// 监听地址
    pub bind_address: String,
    /// 监听端口
    pub port: u16,
    /// 最大节点数
    pub max_nodes: usize,
    /// 心跳超时（秒）
    pub heartbeat_timeout_secs: u64,
    /// 任务超时（秒）
    pub task_timeout_secs: u64,
    /// 最大重试次数
    pub max_retries: u32,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            hub_id: "default-hub".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_nodes: 100,
            heartbeat_timeout_secs: 30,
            task_timeout_secs: 300,
            max_retries: 3,
        }
    }
}

/// Hub 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubStats {
    /// Hub ID
    pub hub_id: String,
    /// 运行时间
    pub uptime_secs: u64,
    /// 节点统计
    pub nodes: NodeManagerStats,
    /// 调度器统计
    pub scheduler: SchedulerStats,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 云端中枢
///
/// 整合 3.x 模块和 4.0 新增能力：
/// - 复用 `uhorse-llm` 进行模型管理
/// - 复用 `uhorse-agent` 进行 Agent 编排
/// - 复用 `uhorse-channel` 进行多通道消息接入
/// - 新增节点管理和任务调度（4.0）
#[derive(Debug)]
pub struct Hub {
    /// 配置
    config: HubConfig,
    /// 启动时间
    started_at: DateTime<Utc>,
    /// 节点管理器
    node_manager: Arc<NodeManager>,
    /// 任务调度器
    task_scheduler: Arc<TaskScheduler>,
    /// 消息路由器
    message_router: Arc<MessageRouter>,
    /// 安全管理器
    security_manager: Option<Arc<SecurityManager>>,
    /// 关闭信号
    shutdown_tx: broadcast::Sender<()>,
}

impl Hub {
    /// 创建新的 Hub
    pub fn new(config: HubConfig) -> (Self, mpsc::Receiver<crate::task_scheduler::TaskResult>) {
        Self::new_with_components(config, None, None, vec![])
    }

    /// 创建带安全配置的 Hub
    pub fn new_with_security(
        config: HubConfig,
        security_manager: Option<Arc<SecurityManager>>,
    ) -> (Self, mpsc::Receiver<crate::task_scheduler::TaskResult>) {
        Self::new_with_components(config, security_manager, None, vec![])
    }

    /// 创建带完整组件的 Hub
    pub fn new_with_components(
        config: HubConfig,
        security_manager: Option<Arc<SecurityManager>>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        notification_bindings: Vec<DingTalkNotificationBinding>,
    ) -> (Self, mpsc::Receiver<crate::task_scheduler::TaskResult>) {
        let node_manager = Arc::new(NodeManager::new(
            config.max_nodes,
            config.heartbeat_timeout_secs,
        ));

        let (task_scheduler, task_result_rx) = TaskScheduler::new(
            node_manager.clone(),
            config.max_retries,
            config.task_timeout_secs,
        );
        let task_scheduler = Arc::new(task_scheduler);

        let message_router = Arc::new(MessageRouter::new(
            node_manager.clone(),
            task_scheduler.clone(),
            dingtalk_channel,
            notification_bindings,
        ));

        let (shutdown_tx, _) = broadcast::channel(1);

        (
            Self {
                config,
                started_at: Utc::now(),
                node_manager,
                task_scheduler,
                message_router,
                security_manager,
                shutdown_tx,
            },
            task_result_rx,
        )
    }

    /// 启动 Hub
    pub async fn start(&self) -> HubResult<()> {
        info!(
            "Starting Hub {} on {}:{}",
            self.config.hub_id, self.config.bind_address, self.config.port
        );
        self.start_background_tasks().await?;
        info!("Hub {} started successfully", self.config.hub_id);
        Ok(())
    }

    /// 启动后台任务
    async fn start_background_tasks(&self) -> HubResult<()> {
        let node_manager = self.node_manager.clone();
        let task_scheduler = self.task_scheduler.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 检查超时节点
                        let timed_out = node_manager.check_timeouts().await;
                        for node_id in timed_out {
                            warn!("Node {} timed out", node_id);
                        }
                        // 检查超时任务
                        let timed_out_tasks = task_scheduler.check_timeouts().await;
                        for task_id in timed_out_tasks {
                            warn!("Task {} timed out", task_id);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Background tasks stopped");
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    /// 停止 Hub
    pub async fn shutdown(&self) -> HubResult<()> {
        info!("Shutting down Hub {}", self.config.hub_id);
        let _ = self.shutdown_tx.send(());
        info!("Hub {} shutdown complete", self.config.hub_id);
        Ok(())
    }

    /// 处理节点连接
    pub async fn handle_node_connection(
        &self,
        node_id: NodeId,
        name: String,
        capabilities: NodeCapabilities,
        workspace: WorkspaceInfo,
        tags: Vec<String>,
    ) -> HubResult<()> {
        self.node_manager
            .register_node(node_id.clone(), name, capabilities, workspace, tags)
            .await?;
        info!("Node {} connected", node_id);
        self.dispatch_pending_tasks().await?;
        Ok(())
    }

    /// 处理节点断开
    pub async fn handle_node_disconnect(&self, node_id: &NodeId) -> HubResult<()> {
        self.node_manager.unregister_node(node_id).await?;
        info!("Node {} disconnected", node_id);
        Ok(())
    }

    /// 处理来自节点的消息
    pub async fn handle_node_message(&self, node_id: &NodeId, message: NodeToHub) -> HubResult<()> {
        let should_dispatch = matches!(
            message,
            NodeToHub::TaskResult { .. }
                | NodeToHub::Heartbeat { .. }
                | NodeToHub::Error {
                    task_id: Some(_),
                    ..
                }
        );

        self.message_router
            .route_node_message(node_id, message, self.security_manager.as_deref())
            .await?;

        if should_dispatch {
            self.dispatch_pending_tasks().await?;
        }

        Ok(())
    }

    /// 提交任务
    pub async fn submit_task(
        &self,
        command: Command,
        context: TaskContext,
        priority: Priority,
        required_capabilities: Option<NodeCapabilities>,
        required_tags: Vec<String>,
        workspace_hint: Option<String>,
    ) -> HubResult<TaskId> {
        let task_id = self
            .task_scheduler
            .submit_task(
                command,
                context,
                priority,
                required_capabilities,
                required_tags,
                workspace_hint,
            )
            .await?;

        self.dispatch_pending_tasks().await?;

        Ok(task_id)
    }

    /// 触发待调度任务分发
    pub async fn dispatch_pending_tasks(&self) -> HubResult<()> {
        loop {
            let senders = self.message_router.node_senders().read().await.clone();
            if senders.is_empty() {
                return Ok(());
            }

            if self.task_scheduler.schedule_next(&senders).await?.is_none() {
                return Ok(());
            }
        }
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &TaskId) -> Option<TaskStatusInfo> {
        self.task_scheduler.get_task_status(task_id).await
    }

    pub async fn get_completed_task(&self, task_id: &TaskId) -> Option<CompletedTask> {
        self.task_scheduler.get_completed_task(task_id).await
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &TaskId, reason: &str) -> HubResult<()> {
        self.task_scheduler.cancel_task(task_id).await
    }

    /// 获取所有节点
    pub async fn get_all_nodes(&self) -> Vec<crate::node_manager::NodeInfo> {
        self.node_manager.get_all_nodes().await
    }

    /// 获取在线节点
    pub async fn get_online_nodes(&self) -> Vec<crate::node_manager::NodeInfo> {
        self.node_manager.get_online_nodes().await
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> HubStats {
        HubStats {
            hub_id: self.config.hub_id.clone(),
            uptime_secs: (Utc::now() - self.started_at).num_seconds() as u64,
            nodes: self.node_manager.get_stats().await,
            scheduler: self.task_scheduler.get_stats().await,
            updated_at: Utc::now(),
        }
    }

    /// 获取 Hub ID
    pub fn hub_id(&self) -> &str {
        &self.config.hub_id
    }

    /// 获取配置
    pub fn config(&self) -> &HubConfig {
        &self.config
    }

    /// 获取节点管理器（供内部使用）
    pub fn node_manager(&self) -> Arc<NodeManager> {
        self.node_manager.clone()
    }

    /// 获取任务调度器（供内部使用）
    pub fn task_scheduler(&self) -> Arc<TaskScheduler> {
        self.task_scheduler.clone()
    }

    /// 获取消息路由器（供内部使用）
    pub fn message_router(&self) -> Arc<MessageRouter> {
        self.message_router.clone()
    }

    /// 获取安全管理器
    pub fn security_manager(&self) -> Option<Arc<SecurityManager>> {
        self.security_manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use uhorse_protocol::{
        Command, FileCommand, MessageId, NotificationEvent, NotificationEventKind, SessionId,
        TaskStatus, UserId,
    };

    #[test]
    fn test_hub_config_default() {
        let config = HubConfig::default();
        assert_eq!(config.hub_id, "default-hub");
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_nodes, 100);
    }

    #[tokio::test]
    async fn test_hub_creation() {
        let config = HubConfig::default();
        let (hub, _rx) = Hub::new(config);
        assert_eq!(hub.hub_id(), "default-hub");
    }

    #[tokio::test]
    async fn test_notification_event_does_not_dispatch_pending_tasks() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let node_id = NodeId::from_string("node-1");

        hub.handle_node_connection(
            node_id.clone(),
            "Test Node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                name: "workspace".to_string(),
                path: "/tmp/workspace".to_string(),
                read_only: false,
                allowed_patterns: vec![],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();

        let task_id = hub
            .submit_task(
                Command::File(FileCommand::Exists {
                    path: "/tmp/workspace/test.txt".to_string(),
                }),
                TaskContext::new(
                    UserId::from_string("user-1"),
                    SessionId::from_string("session-1"),
                    "test",
                ),
                Priority::Normal,
                None,
                vec![],
                None,
            )
            .await
            .unwrap();

        let (tx, mut rx) = mpsc::channel(1);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;

        hub.handle_node_message(
            &node_id,
            NodeToHub::NotificationEvent {
                message_id: MessageId::new(),
                node_id: node_id.clone(),
                event: NotificationEvent::new(
                    NotificationEventKind::Info,
                    "Hub 同步通知",
                    "测试通知内容",
                    false,
                ),
            },
        )
        .await
        .unwrap();

        let status = hub.get_task_status(&task_id).await.unwrap();
        assert!(matches!(status.status, TaskStatus::Queued));
        assert!(timeout(Duration::from_millis(200), rx.recv()).await.is_err());
    }
}
