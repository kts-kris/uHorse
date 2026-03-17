//! 消息路由器
//!
//! 负责 Hub 与 Node 之间的消息路由，复用 uhorse-channel 的多通道能力

use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{HubToNode, NodeToHub, NodeId, TaskId};

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
}

impl MessageRouter {
    /// 创建新的消息路由器
    pub fn new(node_manager: Arc<NodeManager>, task_scheduler: Arc<TaskScheduler>) -> Self {
        Self {
            node_manager,
            task_scheduler,
        }
    }

    /// 处理来自节点的消息
    pub async fn route_node_message(&self, node_id: &NodeId, message: NodeToHub) -> HubResult<()> {
        match message {
            NodeToHub::Heartbeat {
                status,
                load,
                ..
            } => {
                debug!("Received heartbeat from node {}", node_id);
                self.node_manager.update_heartbeat(node_id, status, load).await?;
            }

            NodeToHub::TaskProgress {
                task_id,
                progress,
                message: msg,
                ..
            } => {
                info!(
                    "Task {} progress on node {}: {:.0}% - {}",
                    task_id, node_id, progress * 100.0, msg
                );
            }

            NodeToHub::TaskResult {
                task_id,
                result,
                ..
            } => {
                let success = result.success;
                let error_msg = result.error.as_ref().map(|e| e.message.clone());
                if success {
                    info!("Task {} completed on node {}", task_id, node_id);
                } else {
                    warn!("Task {} failed on node {}: {:?}", task_id, node_id, error_msg);
                }
                self.task_scheduler.complete_task(&task_id, node_id, success, error_msg).await?;
            }

            NodeToHub::Error {
                task_id,
                error,
                ..
            } => {
                if let Some(tid) = task_id {
                    error!("Error from node {} for task {}: {:?}", node_id, tid, error);
                    self.task_scheduler.complete_task(&tid, node_id, false, Some(error.message.clone())).await?;
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
                self.node_manager.unregister_node(&unregister_node_id).await?;
            }

            _ => {
                debug!("Unhandled message from node {}: {:?}", node_id, message);
            }
        }

        Ok(())
    }

    /// 向节点发送消息
    pub async fn send_to_node(&self, node_id: &NodeId, message: HubToNode, sender: &mpsc::Sender<HubToNode>) -> HubResult<()> {
        self.node_manager.send_to_node(node_id, message, sender).await
    }

    /// 广播消息到所有在线节点
    pub async fn broadcast(&self, message: HubToNode, senders: &HashMap<NodeId, mpsc::Sender<HubToNode>>) -> HubResult<usize> {
        use std::collections::HashMap;

        let nodes = self.node_manager.get_online_nodes().await;
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
    pub async fn cancel_task(&self, task_id: &TaskId, reason: &str, senders: &HashMap<NodeId, mpsc::Sender<HubToNode>>) -> HubResult<()> {
        use std::collections::HashMap;
        use uhorse_protocol::MessageId;

        // 先从任务调度器获取任务状态
        if let Some(status) = self.task_scheduler.get_task_status(task_id).await {
            if let Some(node_id) = status.node_id {
                // 发送取消命令到节点
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

// 导入 HashMap
use std::collections::HashMap;
