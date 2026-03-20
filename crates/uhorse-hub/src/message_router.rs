//! 消息路由器
//!
//! 负责 Hub 与 Node 之间的消息路由，复用 uhorse-channel 的多通道能力

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{CommandResult, ErrorSource, ExecutionError, HubToNode, NodeId, NodeToHub, TaskId};

use crate::error::{HubError, HubResult};
use crate::node_manager::NodeManager;
use crate::task_scheduler::TaskScheduler;
use tokio::sync::mpsc;

/// 消息路由器
///
/// 整合节点管理和任务调度，处理 Hub-Node 之间的消息路由
#[derive(Debug)]
pub struct MessageRouter {
    /// 节点管理器
    node_manager: Arc<NodeManager>,
    /// 任务调度器
    task_scheduler: Arc<TaskScheduler>,
    /// 节点消息发送器映射 (用于 WebSocket 连接)
    node_senders: Arc<RwLock<HashMap<NodeId, mpsc::Sender<HubToNode>>>>,
}

impl MessageRouter {
    /// 创建新的消息路由器
    pub fn new(node_manager: Arc<NodeManager>, task_scheduler: Arc<TaskScheduler>) -> Self {
        Self {
            node_manager,
            task_scheduler,
            node_senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册节点的消息发送器 (WebSocket 连接时调用)
    pub async fn register_node_sender(&self, node_id: NodeId, sender: mpsc::Sender<HubToNode>) {
        let mut senders = self.node_senders.write().await;
        senders.insert(node_id.clone(), sender);
        info!("Registered message sender for node {}", node_id);
    }

    /// 注销节点的消息发送器 (WebSocket 断开时调用)
    pub async fn unregister_node_sender(&self, node_id: &NodeId) {
        let mut senders = self.node_senders.write().await;
        if senders.remove(node_id).is_some() {
            info!("Unregistered message sender for node {}", node_id);
        }
    }

    /// 获取节点发送器的引用 (用于内部调用)
    pub fn node_senders(&self) -> Arc<RwLock<HashMap<NodeId, mpsc::Sender<HubToNode>>>> {
        self.node_senders.clone()
    }

    /// 处理来自节点的消息
    pub async fn route_node_message(&self, node_id: &NodeId, message: NodeToHub) -> HubResult<()> {
        match message {
            NodeToHub::Heartbeat { status, load, .. } => {
                debug!("Received heartbeat from node {}", node_id);
                self.node_manager
                    .update_heartbeat(node_id, status, load)
                    .await?;
            }

            NodeToHub::TaskProgress {
                task_id,
                progress,
                message: msg,
                ..
            } => {
                info!(
                    "Task {} progress on node {}: {:.0}% - {}",
                    task_id,
                    node_id,
                    progress * 100.0,
                    msg
                );
            }

            NodeToHub::TaskResult {
                task_id, result, ..
            } => {
                if result.success {
                    info!("Task {} completed on node {}", task_id, node_id);
                } else {
                    warn!(
                        "Task {} failed on node {}: {:?}",
                        task_id,
                        node_id,
                        result.error.as_ref().map(|e| e.message.as_str())
                    );
                }
                self.task_scheduler
                    .complete_task(&task_id, node_id, result)
                    .await?;
            }

            NodeToHub::Error { task_id, error, .. } => {
                if let Some(tid) = task_id {
                    error!("Error from node {} for task {}: {:?}", node_id, tid, error);
                    self.task_scheduler
                        .complete_task(
                            &tid,
                            node_id,
                            CommandResult::failure(
                                ExecutionError::new(
                                    error.code,
                                    error.message,
                                    ErrorSource::Executor,
                                ),
                            ),
                        )
                        .await?;
                } else {
                    error!("Error from node {}: {:?}", node_id, error);
                }
            }

            NodeToHub::ApprovalRequest {
                request_id,
                task_id,
                command,
                reason,
                ..
            } => {
                info!(
                    "Approval request {} from node {} for task {} command: {}",
                    request_id, node_id, task_id, reason
                );
                // TODO: 集成 uhorse-security 的审批流程
            }

            NodeToHub::Unregister {
                node_id: unregister_node_id,
                reason,
                ..
            } => {
                info!("Node {} unregistering: {}", unregister_node_id, reason);
                self.node_manager
                    .unregister_node(&unregister_node_id)
                    .await?;
            }

            _ => {
                debug!("Unhandled message from node {}: {:?}", node_id, message);
            }
        }

        Ok(())
    }

    /// 向节点发送消息
    pub async fn send_to_node(
        &self,
        node_id: &NodeId,
        message: HubToNode,
        sender: &mpsc::Sender<HubToNode>,
    ) -> HubResult<()> {
        self.node_manager
            .send_to_node(node_id, message, sender)
            .await
    }

    /// 广播消息到所有在线节点
    pub async fn broadcast(&self, message: HubToNode) -> HubResult<usize> {
        let nodes = self.node_manager.get_online_nodes().await;
        let senders = self.node_senders.read().await;
        let mut success_count = 0;

        for node in &nodes {
            if let Some(sender) = senders.get(&node.node_id) {
                if sender.send(message.clone()).await.is_ok() {
                    success_count += 1;
                }
            }
        }

        Ok(success_count)
    }

    /// 请求节点取消任务
    pub async fn cancel_task(&self, task_id: &TaskId, reason: &str) -> HubResult<()> {
        use uhorse_protocol::MessageId;

        // 先从任务调度器获取任务状态
        if let Some(status) = self.task_scheduler.get_task_status(task_id).await {
            if let Some(node_id) = status.node_id {
                // 发送取消命令到节点
                let senders = self.node_senders.read().await;
                if let Some(sender) = senders.get(&node_id) {
                    let cancellation = HubToNode::TaskCancellation {
                        message_id: MessageId::new(),
                        task_id: task_id.clone(),
                        reason: reason.to_string(),
                    };
                    sender.send(cancellation).await.map_err(|e| {
                        HubError::Communication(format!("Failed to send cancellation: {}", e))
                    })?;
                }
            }
        }

        // 从调度器中取消任务
        self.task_scheduler.cancel_task(task_id).await?;

        Ok(())
    }
}
