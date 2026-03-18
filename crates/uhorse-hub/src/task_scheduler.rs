//! 任务调度器
//!
//! 复用 uhorse-scheduler 的调度能力，添加 Hub-Node 特有的任务分发逻辑

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uhorse_protocol::{
    Command, HubToNode, MessageId, NodeCapabilities, NodeId, Priority, TaskContext, TaskId,
    TaskStatus, TaskStatus as ProtocolTaskStatus, WorkspaceInfo,
};

use crate::error::{HubError, HubResult};
use crate::node_manager::NodeManager;

/// 队列中的任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    /// 任务 ID
    pub task_id: TaskId,
    /// 命令
    pub command: Command,
    /// 上下文
    pub context: TaskContext,
    /// 优先级
    pub priority: Priority,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 重试次数
    pub retry_count: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 要求的节点能力
    pub required_capabilities: Option<NodeCapabilities>,
    /// 要求的标签
    pub required_tags: Vec<String>,
    /// 工作空间提示
    pub workspace_hint: Option<String>,
}

/// 运行中的任务
#[derive(Debug)]
pub struct RunningTask {
    /// 任务 ID
    pub task_id: TaskId,
    /// 命令
    pub command: Command,
    /// 上下文
    pub context: TaskContext,
    /// 执行节点 ID
    pub node_id: NodeId,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 超时时间
    pub timeout_at: DateTime<Utc>,
    /// 重试次数
    pub retry_count: u32,
}

/// 已完成的任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTask {
    /// 任务 ID
    pub task_id: TaskId,
    /// 命令
    pub command: Command,
    /// 上下文
    pub context: TaskContext,
    /// 执行节点 ID
    pub node_id: NodeId,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: DateTime<Utc>,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
}

/// 任务结果
#[derive(Debug)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: TaskId,
    /// 节点 ID
    pub node_id: NodeId,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
}

/// 调度任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// 任务 ID
    pub task_id: TaskId,
    /// 目标节点 ID
    pub node_id: NodeId,
    /// 调度时间
    pub scheduled_at: DateTime<Utc>,
}

/// 任务状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusInfo {
    /// 任务 ID
    pub task_id: TaskId,
    /// 状态
    pub status: TaskStatus,
    /// 执行节点 ID
    pub node_id: Option<NodeId>,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息
    pub error: Option<String>,
}

/// 调度器统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulerStats {
    /// 待调度任务数
    pub pending_tasks: usize,
    /// 运行中任务数
    pub running_tasks: usize,
    /// 已完成任务数
    pub completed_tasks: usize,
    /// 失败任务数
    pub failed_tasks: usize,
}

/// 任务调度器
#[derive(Debug)]
pub struct TaskScheduler {
    /// 节点管理器
    node_manager: Arc<NodeManager>,
    /// 待调度任务队列（按优先级分组）
    pending_tasks: Arc<RwLock<HashMap<Priority, Vec<QueuedTask>>>>,
    /// 运行中的任务
    running_tasks: Arc<RwLock<HashMap<TaskId, RunningTask>>>,
    /// 已完成的任务（保留一段时间用于查询）
    completed_tasks: Arc<RwLock<HashMap<TaskId, CompletedTask>>>,
    /// 任务计数器
    task_counter: AtomicU64,
    /// 最大重试次数
    max_retries: u32,
    /// 任务超时时间（秒）
    task_timeout_secs: u64,
    /// 任务结果发送通道
    result_tx: mpsc::Sender<TaskResult>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new(
        node_manager: Arc<NodeManager>,
        max_retries: u32,
        task_timeout_secs: u64,
    ) -> (Self, mpsc::Receiver<TaskResult>) {
        let (result_tx, result_rx) = mpsc::channel(1000);

        let mut pending_tasks = HashMap::new();
        for priority in [
            Priority::Critical,
            Priority::Urgent,
            Priority::High,
            Priority::Normal,
            Priority::Low,
            Priority::Background,
        ] {
            pending_tasks.insert(priority, Vec::new());
        }

        (
            Self {
                node_manager,
                pending_tasks: Arc::new(RwLock::new(pending_tasks)),
                running_tasks: Arc::new(RwLock::new(HashMap::new())),
                completed_tasks: Arc::new(RwLock::new(HashMap::new())),
                task_counter: AtomicU64::new(0),
                max_retries,
                task_timeout_secs,
                result_tx,
            },
            result_rx,
        )
    }

    /// 生成新的任务 ID
    pub fn generate_task_id(&self) -> TaskId {
        let id = self.task_counter.fetch_add(1, Ordering::SeqCst);
        TaskId::from_string(format!("task-{}", id))
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
        let task_id = self.generate_task_id();

        let task = QueuedTask {
            task_id: task_id.clone(),
            command,
            context,
            priority,
            created_at: Utc::now(),
            retry_count: 0,
            max_retries: self.max_retries,
            required_capabilities,
            required_tags,
            workspace_hint,
        };

        // 添加到待调度队列
        {
            let mut pending = self.pending_tasks.write().await;
            if let Some(queue) = pending.get_mut(&priority) {
                queue.push(task);
                info!("Task {} submitted with priority {:?}", task_id, priority);
            }
        }

        Ok(task_id)
    }

    /// 调度下一个任务
    pub async fn schedule_next(
        &self,
        sender: &mpsc::Sender<HubToNode>,
    ) -> HubResult<Option<ScheduledTask>> {
        let priorities = [
            Priority::Critical,
            Priority::Urgent,
            Priority::High,
            Priority::Normal,
            Priority::Low,
            Priority::Background,
        ];

        for priority in priorities {
            let task = {
                let mut pending = self.pending_tasks.write().await;
                if let Some(queue) = pending.get_mut(&priority) {
                    if !queue.is_empty() {
                        Some(queue.remove(0))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(task) = task {
                // 选择合适的节点
                if let Some(node) = self
                    .node_manager
                    .select_node(
                        task.required_capabilities.as_ref(),
                        &task.required_tags,
                        task.workspace_hint.as_deref(),
                    )
                    .await
                {
                    // 发送任务到节点
                    let assignment = HubToNode::TaskAssignment {
                        message_id: MessageId::new(),
                        task_id: task.task_id.clone(),
                        command: task.command.clone(),
                        priority: task.priority,
                        deadline: Some(
                            Utc::now() + chrono::Duration::seconds(self.task_timeout_secs as i64),
                        ),
                        context: uhorse_protocol::TaskContext {
                            user_id: task.context.user_id.clone(),
                            session_id: task.context.session_id.clone(),
                            channel: task.context.channel.clone(),
                            intent: task.context.intent.clone(),
                            env: task.context.env.clone(),
                            created_at: task.context.created_at,
                        },
                        retry_count: task.retry_count,
                        max_retries: task.max_retries,
                    };

                    if let Err(e) = self
                        .node_manager
                        .send_to_node(&node.node_id, assignment, sender)
                        .await
                    {
                        warn!("Failed to send task to node: {}", e);

                        // 如果发送失败，将任务放回队列
                        if task.retry_count < task.max_retries {
                            let mut task = task;
                            task.retry_count += 1;
                            let mut pending = self.pending_tasks.write().await;
                            if let Some(queue) = pending.get_mut(&priority) {
                                queue.insert(0, task);
                            }
                        }
                        continue;
                    }

                    // 记录运行中的任务
                    let running = RunningTask {
                        task_id: task.task_id.clone(),
                        command: task.command,
                        context: task.context,
                        node_id: node.node_id.clone(),
                        started_at: Utc::now(),
                        timeout_at: Utc::now()
                            + chrono::Duration::seconds(self.task_timeout_secs as i64),
                        retry_count: task.retry_count,
                    };

                    {
                        let mut running_tasks = self.running_tasks.write().await;
                        running_tasks.insert(task.task_id.clone(), running);
                    }

                    let scheduled = ScheduledTask {
                        task_id: task.task_id.clone(),
                        node_id: node.node_id.clone(),
                        scheduled_at: Utc::now(),
                    };

                    info!("Task {} scheduled to node {}", task.task_id, node.node_id);
                    return Ok(Some(scheduled));
                } else {
                    // 没有合适的节点，将任务放回队列
                    let mut pending = self.pending_tasks.write().await;
                    if let Some(queue) = pending.get_mut(&priority) {
                        queue.insert(0, task);
                    }
                    debug!("No suitable node available for task");
                }
            }
        }

        Ok(None)
    }

    /// 处理任务完成
    pub async fn complete_task(
        &self,
        task_id: &TaskId,
        node_id: &NodeId,
        success: bool,
        error: Option<String>,
    ) -> HubResult<()> {
        let mut running_tasks = self.running_tasks.write().await;

        if let Some(running) = running_tasks.remove(task_id) {
            let completed = CompletedTask {
                task_id: task_id.clone(),
                command: running.command,
                context: running.context,
                node_id: node_id.clone(),
                started_at: running.started_at,
                completed_at: Utc::now(),
                success,
                error: error.clone(),
            };

            // 添加到已完成任务列表
            {
                let mut completed_tasks = self.completed_tasks.write().await;
                completed_tasks.insert(task_id.clone(), completed);

                // 清理旧任务（保留最近 500 个）
                if completed_tasks.len() > 500 {
                    let tasks: Vec<_> = completed_tasks.iter().collect();
                    let mut sorted_tasks: Vec<_> = tasks.into_iter().collect();
                    sorted_tasks.sort_by_key(|(_, t)| t.completed_at);
                    let ids_to_remove: Vec<_> = sorted_tasks
                        .iter()
                        .take(250)
                        .map(|(id, _)| (*id).clone())
                        .collect();
                    for id in ids_to_remove {
                        completed_tasks.remove(&id);
                    }
                }
            }

            // 发送结果通知
            let result = TaskResult {
                task_id: task_id.clone(),
                node_id: node_id.clone(),
                success,
                error,
            };

            if self.result_tx.send(result).await.is_err() {
                warn!("Failed to send task result notification");
            }

            info!(
                "Task {} completed on node {}: {}",
                task_id,
                node_id,
                if success { "success" } else { "failed" }
            );
        }

        Ok(())
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &TaskId) -> HubResult<()> {
        // 检查待调度队列
        {
            let mut pending = self.pending_tasks.write().await;
            for queue in pending.values_mut() {
                if let Some(pos) = queue.iter().position(|t| &t.task_id == task_id) {
                    queue.remove(pos);
                    info!("Task {} removed from queue", task_id);
                    return Ok(());
                }
            }
        }

        Err(HubError::Task(format!("Task not found: {}", task_id)))
    }

    /// 检查超时任务
    pub async fn check_timeouts(&self) -> Vec<TaskId> {
        let mut running_tasks = self.running_tasks.write().await;
        let now = Utc::now();
        let mut timed_out = Vec::new();

        for (task_id, running) in running_tasks.iter() {
            if now > running.timeout_at {
                timed_out.push(task_id.clone());
            }
        }

        for task_id in &timed_out {
            if let Some(running) = running_tasks.remove(task_id) {
                warn!("Task {} timed out on node {}", task_id, running.node_id);

                let completed = CompletedTask {
                    task_id: task_id.clone(),
                    command: running.command,
                    context: running.context,
                    node_id: running.node_id.clone(),
                    started_at: running.started_at,
                    completed_at: now,
                    success: false,
                    error: Some("Task timed out".to_string()),
                };

                let mut completed_tasks = self.completed_tasks.write().await;
                completed_tasks.insert(task_id.clone(), completed);
            }
        }

        timed_out
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &TaskId) -> Option<TaskStatusInfo> {
        // 检查运行中的任务
        {
            let running_tasks = self.running_tasks.read().await;
            if let Some(running) = running_tasks.get(task_id) {
                return Some(TaskStatusInfo {
                    task_id: task_id.clone(),
                    status: TaskStatus::Running,
                    node_id: Some(running.node_id.clone()),
                    started_at: Some(running.started_at),
                    completed_at: None,
                    error: None,
                });
            }
        }

        // 检查已完成的任务
        {
            let completed_tasks = self.completed_tasks.read().await;
            if let Some(completed) = completed_tasks.get(task_id) {
                return Some(TaskStatusInfo {
                    task_id: task_id.clone(),
                    status: if completed.success {
                        TaskStatus::Completed
                    } else {
                        TaskStatus::Failed
                    },
                    node_id: Some(completed.node_id.clone()),
                    started_at: Some(completed.started_at),
                    completed_at: Some(completed.completed_at),
                    error: completed.error.clone(),
                });
            }
        }

        // 检查待调度队列
        {
            let pending = self.pending_tasks.read().await;
            for queue in pending.values() {
                if let Some(task) = queue.iter().find(|t| &t.task_id == task_id) {
                    return Some(TaskStatusInfo {
                        task_id: task_id.clone(),
                        status: TaskStatus::Queued,
                        node_id: None,
                        started_at: None,
                        completed_at: None,
                        error: None,
                    });
                }
            }
        }

        None
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> SchedulerStats {
        let pending = self.pending_tasks.read().await;
        let running_tasks = self.running_tasks.read().await;
        let completed_tasks = self.completed_tasks.read().await;

        let mut pending_count = 0;
        for queue in pending.values() {
            pending_count += queue.len();
        }

        let mut completed_count = 0;
        let mut failed_count = 0;
        for task in completed_tasks.values() {
            if task.success {
                completed_count += 1;
            } else {
                failed_count += 1;
            }
        }

        SchedulerStats {
            pending_tasks: pending_count,
            running_tasks: running_tasks.len(),
            completed_tasks: completed_count,
            failed_tasks: failed_count,
        }
    }
}
