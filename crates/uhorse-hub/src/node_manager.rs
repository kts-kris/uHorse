//! 节点管理器
//!
//! 管理连接的本地节点，复用 uhorse-discovery 的服务发现能力

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uhorse_protocol::{HubToNode, LoadInfo, NodeCapabilities, NodeId, NodeStatus, WorkspaceInfo};

use crate::error::{HubError, HubResult};

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// 节点 ID
    pub node_id: NodeId,
    /// 节点名称
    pub name: String,
    /// 节点状态
    pub state: NodeState,
    /// 能力
    pub capabilities: NodeCapabilities,
    /// 工作空间信息
    pub workspace: WorkspaceInfo,
    /// 标签
    pub tags: Vec<String>,
    /// 最后心跳时间
    pub last_heartbeat: DateTime<Utc>,
    /// 负载信息
    pub load: LoadInfo,
    /// 注册时间
    pub registered_at: DateTime<Utc>,
    /// 当前任务数
    pub current_tasks: usize,
    /// 完成的任务数
    pub completed_tasks: u64,
    /// 失败的任务数
    pub failed_tasks: u64,
}

/// 节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// 在线
    Online,
    /// 离线
    Offline,
    /// 忙碌
    Busy,
    /// 维护中
    Maintenance,
}

/// 节点管理器统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeManagerStats {
    /// 总节点数
    pub total_nodes: usize,
    /// 在线节点数
    pub online_nodes: usize,
    /// 离线节点数
    pub offline_nodes: usize,
    /// 忙碌节点数
    pub busy_nodes: usize,
}

/// 节点管理器
#[derive(Debug)]
pub struct NodeManager {
    /// 已注册的节点
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
    /// 心跳超时（秒）
    heartbeat_timeout_secs: u64,
    /// 最大节点数
    max_nodes: usize,
}

impl NodeManager {
    /// 创建新的节点管理器
    pub fn new(max_nodes: usize, heartbeat_timeout_secs: u64) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_timeout_secs,
            max_nodes,
        }
    }

    /// 注册节点
    pub async fn register_node(
        &self,
        node_id: NodeId,
        name: String,
        capabilities: NodeCapabilities,
        workspace: WorkspaceInfo,
        tags: Vec<String>,
    ) -> HubResult<()> {
        let mut nodes = self.nodes.write().await;

        if nodes.len() >= self.max_nodes && !nodes.contains_key(&node_id) {
            return Err(HubError::NodeLimitReached);
        }

        let now = Utc::now();
        let info = NodeInfo {
            node_id: node_id.clone(),
            name,
            state: NodeState::Online,
            capabilities,
            workspace,
            tags,
            last_heartbeat: now,
            load: LoadInfo::default(),
            registered_at: now,
            current_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
        };

        nodes.insert(node_id.clone(), info);
        info!("Node registered: {}", node_id);
        Ok(())
    }

    /// 注销节点
    pub async fn unregister_node(&self, node_id: &NodeId) -> HubResult<()> {
        let mut nodes = self.nodes.write().await;

        if nodes.remove(node_id).is_some() {
            info!("Node unregistered: {}", node_id);
            Ok(())
        } else {
            Err(HubError::NodeNotFound(node_id.clone()))
        }
    }

    /// 更新节点心跳
    pub async fn update_heartbeat(
        &self,
        node_id: &NodeId,
        status: NodeStatus,
        load: LoadInfo,
    ) -> HubResult<()> {
        let mut nodes = self.nodes.write().await;

        if let Some(info) = nodes.get_mut(node_id) {
            info.last_heartbeat = Utc::now();
            info.load = load;
            info.state = if status.current_tasks >= info.capabilities.max_concurrent_tasks {
                NodeState::Busy
            } else {
                NodeState::Online
            };
            info.current_tasks = status.current_tasks;
            debug!("Node heartbeat updated: {}", node_id);
            Ok(())
        } else {
            Err(HubError::NodeNotFound(node_id.clone()))
        }
    }

    /// 获取节点信息
    pub async fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// 获取所有在线节点
    pub async fn get_online_nodes(&self) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|n| n.state == NodeState::Online)
            .cloned()
            .collect()
    }

    /// 获取所有节点
    pub async fn get_all_nodes(&self) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// 选择最适合执行任务的节点
    pub async fn select_node(
        &self,
        required_capabilities: Option<&NodeCapabilities>,
        tags: &[String],
        workspace_hint: Option<&str>,
    ) -> Option<NodeInfo> {
        let nodes = self.nodes.read().await;

        let mut candidates: Vec<&NodeInfo> = nodes
            .values()
            .filter(|n| {
                if n.state != NodeState::Online {
                    return false;
                }

                if let Some(required) = required_capabilities {
                    if !n.capabilities.meets(required) {
                        return false;
                    }
                }

                if !tags.is_empty() {
                    let has_tag = tags.iter().any(|t| n.tags.contains(t));
                    if !has_tag {
                        return false;
                    }
                }

                if let Some(hint) = workspace_hint {
                    if !n.workspace.path.contains(hint) {
                        return false;
                    }
                }

                true
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // 按负载排序，选择负载最低的
        candidates.sort_by(|a, b| {
            let a_load = a.load.score();
            let b_load = b.load.score();
            a_load
                .partial_cmp(&b_load)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.first().map(|n| (*n).clone())
    }

    /// 检查超时节点
    pub async fn check_timeouts(&self) -> Vec<NodeId> {
        let mut nodes = self.nodes.write().await;
        let now = Utc::now();
        let timeout = chrono::Duration::seconds(self.heartbeat_timeout_secs as i64);

        let mut timed_out = Vec::new();

        for (node_id, info) in nodes.iter_mut() {
            if now - info.last_heartbeat > timeout {
                warn!("Node timed out: {}", node_id);
                info.state = NodeState::Offline;
                timed_out.push(node_id.clone());
            }
        }

        timed_out
    }

    /// 发送消息到节点（通过 sender）
    pub async fn send_to_node(
        &self,
        node_id: &NodeId,
        message: HubToNode,
        sender: &mpsc::Sender<HubToNode>,
    ) -> HubResult<()> {
        sender.send(message).await.map_err(|e| {
            HubError::Communication(format!("Failed to send message to {}: {}", node_id, e))
        })?;
        debug!("Message sent to node: {}", node_id);
        Ok(())
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> NodeManagerStats {
        let nodes = self.nodes.read().await;

        let mut stats = NodeManagerStats::default();
        stats.total_nodes = nodes.len();

        for info in nodes.values() {
            match info.state {
                NodeState::Online => stats.online_nodes += 1,
                NodeState::Offline => stats.offline_nodes += 1,
                NodeState::Busy => stats.busy_nodes += 1,
                NodeState::Maintenance => {}
            }
        }

        stats
    }
}
