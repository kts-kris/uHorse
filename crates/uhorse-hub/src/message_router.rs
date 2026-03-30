//! 消息路由器
//!
//! 负责 Hub 与 Node 之间的消息路由，复用 uhorse-channel 的多通道能力

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_channel::DingTalkChannel;
use uhorse_core::{Channel, MessageContent};
use uhorse_protocol::{
    CommandResult, ErrorSource, ExecutionError, HubToNode, NodeId, NodeToHub,
    NotificationEventKind, TaskId,
};
use uhorse_security::ApprovalLevel;

use crate::error::{HubError, HubResult};
use crate::node_manager::NodeManager;
use crate::notification_binding::NotificationBindingManager;
use crate::security_integration::SecurityManager;
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
    /// DingTalk 通道
    dingtalk_channel: Option<Arc<DingTalkChannel>>,
    /// 节点通知绑定
    notification_bindings: Arc<NotificationBindingManager>,
    /// 节点消息发送器映射 (用于 WebSocket 连接)
    node_senders: Arc<RwLock<HashMap<NodeId, mpsc::Sender<HubToNode>>>>,
}

impl MessageRouter {
    /// 创建新的消息路由器
    pub fn new(
        node_manager: Arc<NodeManager>,
        task_scheduler: Arc<TaskScheduler>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        notification_bindings: Arc<NotificationBindingManager>,
    ) -> Self {
        Self {
            node_manager,
            task_scheduler,
            dingtalk_channel,
            notification_bindings,
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
    pub async fn route_node_message(
        &self,
        node_id: &NodeId,
        message: NodeToHub,
        security_manager: Option<&SecurityManager>,
    ) -> HubResult<()> {
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
                            CommandResult::failure(ExecutionError::new(
                                error.code,
                                error.message,
                                ErrorSource::Executor,
                            )),
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
                context,
                reason,
                timestamp,
                expires_at,
                ..
            } => {
                info!(
                    "Approval request {} from node {} for task {} command: {}",
                    request_id, node_id, task_id, reason
                );

                let security_manager = security_manager.ok_or_else(|| {
                    HubError::Permission("Approval request requires security manager".to_string())
                })?;

                let operation = Self::approval_operation_for_command(&command);
                let ttl_seconds = Self::approval_request_ttl_seconds(timestamp, expires_at);
                if security_manager
                    .operation_approver()
                    .check_idempotency(&request_id, ttl_seconds)
                    .await?
                {
                    debug!(
                        "Approval request {} from node {} already processed",
                        request_id, node_id
                    );
                    return Ok(());
                }

                let metadata = serde_json::json!({
                    "request_id": request_id,
                    "task_id": task_id.as_str(),
                    "node_id": node_id.as_str(),
                    "command_type": format!("{:?}", command.command_type()).to_lowercase(),
                    "command": command,
                    "context": context,
                    "reason": reason,
                    "requested_at": timestamp,
                    "expires_at": expires_at,
                });

                let approval_id = security_manager
                    .operation_approver()
                    .request_approval(node_id, operation, ApprovalLevel::Single, metadata)
                    .await?;

                security_manager
                    .operation_approver()
                    .store_idempotency_response(
                        &request_id,
                        &serde_json::json!({ "approval_id": approval_id }),
                        ttl_seconds,
                    )
                    .await?;

                info!(
                    "Approval request {} from node {} stored as hub approval {}",
                    request_id, node_id, approval_id
                );
            }

            NodeToHub::NotificationEvent { event, .. } => {
                let event_kind = Self::notification_kind_name(event.kind.clone());

                let detail_mode = if event.details_included {
                    "details_included"
                } else {
                    "summary_only"
                };

                let dedupe_key = event.dedupe_key.as_deref().unwrap_or("-");

                if let Err(error) = self.forward_notification_to_dingtalk(node_id, &event).await {
                    warn!(
                        node_id = %node_id,
                        event_id = %event.event_id,
                        event_kind,
                        detail_mode,
                        dedupe_key,
                        title = %event.title,
                        body = %event.body,
                        error = %error,
                        "Failed to mirror node notification event to DingTalk"
                    );
                }
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

    fn approval_operation_for_command(command: &uhorse_protocol::Command) -> &'static str {
        match command.command_type() {
            uhorse_protocol::CommandType::File => "file_delete",
            uhorse_protocol::CommandType::Shell => "system_command",
            uhorse_protocol::CommandType::Api
            | uhorse_protocol::CommandType::Browser
            | uhorse_protocol::CommandType::Database => "network_access",
            uhorse_protocol::CommandType::Code | uhorse_protocol::CommandType::Skill => {
                "config_change"
            }
        }
    }

    fn approval_request_ttl_seconds(
        requested_at: chrono::DateTime<chrono::Utc>,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> u64 {
        expires_at
            .signed_duration_since(requested_at)
            .to_std()
            .map(|duration| duration.as_secs().max(1))
            .unwrap_or(1)
    }

    fn notification_kind_name(kind: NotificationEventKind) -> &'static str {
        match kind {
            NotificationEventKind::Test => "test",
            NotificationEventKind::Info => "info",
            NotificationEventKind::Warn => "warn",
            NotificationEventKind::Error => "error",
        }
    }

    async fn forward_notification_to_dingtalk(
        &self,
        node_id: &NodeId,
        event: &uhorse_protocol::NotificationEvent,
    ) -> HubResult<()> {
        let Some(channel) = self.dingtalk_channel.as_ref() else {
            return Ok(());
        };

        let Some(user_id) = self
            .notification_bindings
            .get_user_id(node_id.as_str())
            .await
        else {
            warn!(
                node_id = %node_id,
                event_id = %event.event_id,
                "Received node notification event but no Hub-side DingTalk recipient binding is configured yet"
            );
            return Ok(());
        };

        channel
            .send_message(
                &user_id,
                &MessageContent::Text(Self::render_notification_message(node_id, event)),
            )
            .await
            .map_err(|error| {
                HubError::Communication(format!("Failed to send DingTalk notification: {}", error))
            })
    }

    fn render_notification_message(
        node_id: &NodeId,
        event: &uhorse_protocol::NotificationEvent,
    ) -> String {
        let kind = match event.kind {
            NotificationEventKind::Test => "测试",
            NotificationEventKind::Info => "信息",
            NotificationEventKind::Warn => "警告",
            NotificationEventKind::Error => "错误",
        };

        format!(
            "[uHorse 节点通知]\n类型：{}\n节点：{}\n标题：{}\n内容：{}",
            kind, node_id, event.title, event.body
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use uhorse_protocol::{MessageId, NotificationEvent};

    #[test]
    fn test_render_notification_message_includes_node_and_content() {
        let node_id = NodeId::from_string("node-desktop-test");
        let event = NotificationEvent::new(NotificationEventKind::Warn, "标题", "内容", true);

        let message = MessageRouter::render_notification_message(&node_id, &event);
        assert!(message.contains("node-desktop-test"));
        assert!(message.contains("标题"));
        assert!(message.contains("内容"));
        assert!(message.contains("警告"));
    }

    #[tokio::test]
    async fn test_notification_without_binding_is_ignored() {
        let node_manager = Arc::new(NodeManager::new(10, 30));
        let (task_scheduler, _rx) = TaskScheduler::new(node_manager.clone(), 3, 300);
        let router = MessageRouter::new(
            node_manager,
            Arc::new(task_scheduler),
            None,
            Arc::new(NotificationBindingManager::default()),
        );
        let node_id = NodeId::from_string("node-desktop-test");
        let event = NotificationEvent::new(NotificationEventKind::Info, "标题", "内容", true);

        router
            .route_node_message(
                &node_id,
                NodeToHub::NotificationEvent {
                    message_id: MessageId::new(),
                    node_id: node_id.clone(),
                    event,
                },
                None,
            )
            .await
            .unwrap();
    }
}
