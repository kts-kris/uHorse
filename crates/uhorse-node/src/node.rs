//! 本地节点
//!
//! 负责管理节点生命周期、处理任务和与 Hub 通信

use crate::connection::{ConnectionConfig, HubConnection};
use crate::error::{NodeError, NodeResult};
use crate::executor::CommandExecutor;
use crate::permission::PermissionManager;
use crate::status::{Metrics, StatusReporter};
use crate::workspace::Workspace;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{
    Command, CommandResult, ErrorSource, ExecutionError, ExecutionMetrics, HubToNode,
    NodeCapabilities, NodeId, NodeStatus, NodeToHub, TaskContext, TaskId,
};

/// 节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// 节点 ID（如果不设置则自动生成）
    #[serde(default)]
    pub node_id: Option<NodeId>,

    /// 节点名称
    pub name: String,

    /// Hub 连接配置
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// 工作空间路径
    pub workspace_path: String,

    /// 心跳间隔（秒）
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// 状态报告间隔（秒）
    #[serde(default = "default_status_interval")]
    pub status_interval_secs: u64,

    /// 最大并发任务数
    #[serde(default = "default_max_tasks")]
    pub max_concurrent_tasks: usize,

    /// 节点能力
    #[serde(default)]
    pub capabilities: NodeCapabilities,

    /// 标签（用于任务路由）
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_status_interval() -> u64 {
    60
}

fn default_max_tasks() -> usize {
    5
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            name: "uHorse-Node".to_string(),
            connection: ConnectionConfig::default(),
            workspace_path: ".".to_string(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            status_interval_secs: default_status_interval(),
            max_concurrent_tasks: default_max_tasks(),
            capabilities: NodeCapabilities::default(),
            tags: vec!["default".to_string()],
        }
    }
}

/// 本地节点
pub struct Node {
    /// 配置
    config: NodeConfig,

    /// 节点 ID
    node_id: NodeId,

    /// 工作空间
    workspace: Arc<Workspace>,

    /// Hub 连接
    connection: HubConnection,

    /// 命令执行器
    executor: Arc<CommandExecutor>,

    /// 权限管理器
    permission_manager: Arc<PermissionManager>,

    /// 状态报告器
    status_reporter: Arc<StatusReporter>,

    /// 指标收集
    metrics: Arc<RwLock<Metrics>>,

    /// 运行任务
    running_tasks: Arc<RwLock<HashMap<TaskId, RunningTask>>>,

    /// 运行标志
    running: Arc<AtomicBool>,

    /// 停止信号
    stop_signal: broadcast::Sender<()>,

    /// 状态更新接收器
    status_rx: broadcast::Receiver<NodeStatus>,
    status_tx: broadcast::Sender<NodeStatus>,
}

/// 运行中的任务
#[derive(Debug)]
struct RunningTask {
    /// 任务 ID
    task_id: TaskId,

    /// 命令
    command: Command,

    /// 上下文
    context: TaskContext,

    /// 开始时间
    started_at: DateTime<Utc>,

    /// 取消信号
    cancel_tx: mpsc::Sender<()>,
}

impl Node {
    /// 创建新的节点
    pub fn new(config: NodeConfig) -> NodeResult<Self> {
        // 创建工作空间
        let workspace = Arc::new(Workspace::new(&config.workspace_path)?);

        // 创建权限管理器
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone()));

        // 创建执行器
        let executor = Arc::new(CommandExecutor::new(
            workspace.clone(),
            permission_manager.clone(),
        ));

        // 创建节点 ID
        let node_id = config.node_id.clone().unwrap_or_else(NodeId::new);

        // 创建状态报告器
        let status_reporter = Arc::new(StatusReporter::new(node_id.clone()));

        // 创建广播通道
        let (stop_signal, _) = broadcast::channel(1);
        let (status_tx, status_rx) = broadcast::channel(16);

        // 创建连接
        let connection = HubConnection::new(
            node_id.clone(),
            config.connection.clone(),
            config.name.clone(),
            workspace.root().to_string_lossy().to_string(),
        );

        Ok(Self {
            config,
            node_id,
            workspace,
            connection,
            executor,
            permission_manager,
            status_reporter,
            metrics: Arc::new(RwLock::new(Metrics::default())),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            stop_signal,
            status_rx,
            status_tx,
        })
    }

    /// 启动节点
    pub async fn start(&mut self) -> NodeResult<()> {
        if self.running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(NodeError::Internal("Node already running".to_string()));
        }

        info!("Starting node: {} ({})", self.config.name, self.node_id);

        // 1. 加载默认权限规则
        self.permission_manager.load_default_rules().await;

        // 2. 连接到 Hub
        let (hub_rx, outbound_tx) = self.connection.start().await?;

        // 3. 启动后台任务
        self.start_status_task();
        self.start_message_handler(hub_rx, outbound_tx);

        info!("Node started successfully");
        Ok(())
    }

    /// 启动状态报告任务
    fn start_status_task(&self) {
        let interval_secs = self.config.status_interval_secs;
        let running = self.running.clone();
        let _status_tx = self.status_tx.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                if !running.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                // 收集并报告状态
                let _m = metrics.read().await.clone();
                // TODO: 发送详细状态报告
            }
        });
    }

    /// 启动消息处理器
    fn start_message_handler(
        &self,
        mut receiver: mpsc::Receiver<HubToNode>,
        outbound_tx: mpsc::Sender<NodeToHub>,
    ) {
        let running = self.running.clone();
        let executor = self.executor.clone();
        let metrics = self.metrics.clone();
        let running_tasks = self.running_tasks.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            loop {
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                tokio::select! {
                    // 接收 Hub 消息
                    msg = receiver.recv() => {
                        match msg {
                            Some(message) => {
                                if let Err(e) = Self::handle_hub_message(
                                    &node_id,
                                    &message,
                                    &executor,
                                    &metrics,
                                    &running_tasks,
                                    &outbound_tx,
                                ).await {
                                    error!("Failed to handle Hub message: {}", e);
                                }
                            }
                            None => {
                                info!("Hub connection closed");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    /// 处理 Hub 消息
    async fn handle_hub_message(
        node_id: &NodeId,
        message: &HubToNode,
        executor: &Arc<CommandExecutor>,
        metrics: &Arc<RwLock<Metrics>>,
        running_tasks: &Arc<RwLock<HashMap<TaskId, RunningTask>>>,
        outbound_tx: &mpsc::Sender<NodeToHub>,
    ) -> NodeResult<()> {
        match message {
            HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                info!("Received task: {}", task_id);
                let started_at = Instant::now();

                let (cancel_tx, _cancel_rx) = mpsc::channel(1);
                {
                    let mut tasks = running_tasks.write().await;
                    tasks.insert(
                        task_id.clone(),
                        RunningTask {
                            task_id: task_id.clone(),
                            command: command.clone(),
                            context: context.clone(),
                            started_at: Utc::now(),
                            cancel_tx,
                        },
                    );
                }

                // 执行任务
                let execution = executor.execute(task_id, command, context).await;
                let duration_ms = started_at.elapsed().as_millis() as u64;

                let result = match execution {
                    Ok(result) => result,
                    Err(error) => Self::command_result_from_node_error(&error, duration_ms),
                };

                // 更新指标
                {
                    let mut m = metrics.write().await;
                    m.record_execution(result.success, duration_ms);
                }

                // 更新 running_tasks
                {
                    let mut tasks = running_tasks.write().await;
                    tasks.remove(task_id);
                }

                let message = NodeToHub::TaskResult {
                    message_id: uhorse_protocol::MessageId::new(),
                    task_id: task_id.clone(),
                    result: result.clone(),
                    metrics: ExecutionMetrics {
                        duration_ms,
                        ..Default::default()
                    },
                };

                outbound_tx.send(message).await.map_err(|e| {
                    NodeError::Connection(format!("Failed to send task result: {}", e))
                })?;

                info!(
                    "Task {} completed on node {}: {}",
                    task_id,
                    node_id,
                    if result.success { "success" } else { "failed" }
                );
            }

            HubToNode::TaskCancellation {
                task_id, reason, ..
            } => {
                info!("Task cancelled: {} - {}", task_id, reason);

                // 取消运行中的任务
                let mut tasks = running_tasks.write().await;
                if let Some(task) = tasks.remove(task_id) {
                    let _ = task.cancel_tx.send(()).await;
                }
            }

            HubToNode::HeartbeatRequest { .. } => {
                debug!("Heartbeat request received");
            }

            HubToNode::ConfigUpdate { .. } => {
                info!("Config update received");
            }

            HubToNode::PermissionUpdate { rules, .. } => {
                info!("Permission update: {} rules", rules.len());
            }

            HubToNode::SkillDeploy { .. } => {
                info!("Skill deploy received");
            }

            HubToNode::SkillRemove { .. } => {
                info!("Skill remove received");
            }

            HubToNode::Disconnect { reason, .. } => {
                warn!("Hub requested disconnect: {}", reason);
            }
        }

        Ok(())
    }

    fn command_result_from_node_error(error: &NodeError, duration_ms: u64) -> CommandResult {
        let execution_error = match error {
            NodeError::Permission(message) => ExecutionError::permission_denied(message.clone()),
            NodeError::Timeout(message) => ExecutionError::timeout(message.clone()),
            NodeError::Execution(message) => ExecutionError::execution_failed(message.clone()),
            NodeError::Workspace(message) => {
                ExecutionError::validation_failed(message.clone())
            }
            NodeError::Config(message) => ExecutionError::validation_failed(message.clone()),
            NodeError::Connection(message) => {
                ExecutionError::new("CONNECTION_FAILED", message.clone(), ErrorSource::External)
            }
            NodeError::Protocol(error) => ExecutionError::new(
                "PROTOCOL_ERROR",
                error.to_string(),
                ErrorSource::Internal,
            ),
            NodeError::Io(error) => {
                ExecutionError::new("IO_ERROR", error.to_string(), ErrorSource::Executor)
            }
            NodeError::Serialization(error) => ExecutionError::new(
                "SERIALIZATION_ERROR",
                error.to_string(),
                ErrorSource::Internal,
            ),
            NodeError::Internal(message) => {
                ExecutionError::new("INTERNAL_ERROR", message.clone(), ErrorSource::Internal)
            }
        };

        CommandResult::failure(execution_error).with_duration(duration_ms)
    }

    /// 停止节点
    pub async fn stop(&self) -> NodeResult<()> {
        if !self
            .running
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(()); // 已经停止
        }

        info!("Stopping node: {}", self.node_id);

        // 发送停止信号
        let _ = self.stop_signal.send(());

        // 停止连接
        self.connection.stop().await;

        info!("Node stopped");
        Ok(())
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// 获取状态流
    pub fn subscribe_status(&self) -> broadcast::Receiver<NodeStatus> {
        self.status_tx.subscribe()
    }

    /// 获取指标
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) {
            warn!("Node dropped while still running");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();
        assert_eq!(config.name, "uHorse-Node");
        assert_eq!(config.max_concurrent_tasks, 5);
    }
}
