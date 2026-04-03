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
    Command, CommandResult, CommandType, ExecutionError, HubToNode, MessageId, NodeCapabilities,
    NodeId, Priority, TaskContext, TaskId, TaskStatus,
};

use crate::error::{HubError, HubResult};
use crate::node_manager::{workspace_matches_hint, NodeManager};

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
    /// 优先级
    pub priority: Priority,
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
    /// 优先级
    pub priority: Priority,
    /// 执行节点 ID
    pub node_id: NodeId,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: DateTime<Utc>,
    /// 终态状态
    pub status: TaskStatus,
    /// 完整执行结果
    pub result: CommandResult,
}

/// 任务结果
#[derive(Debug)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: TaskId,
    /// 节点 ID
    pub node_id: NodeId,
    /// 完整执行结果
    pub result: CommandResult,
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
    /// 命令类型
    pub command_type: Option<CommandType>,
    /// 优先级
    pub priority: Option<Priority>,
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

    fn terminal_status_from_result(result: &CommandResult) -> TaskStatus {
        if result.success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        }
    }

    async fn store_completed_task(&self, completed: CompletedTask) {
        let mut completed_tasks = self.completed_tasks.write().await;
        completed_tasks.insert(completed.task_id.clone(), completed);

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
        senders: &HashMap<NodeId, mpsc::Sender<HubToNode>>,
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
                let execution_workspace_id = task.context.execution_workspace_id.clone();
                let mut candidates = if let Some(workspace_id) = execution_workspace_id.as_deref() {
                    self.node_manager
                        .get_node_by_workspace_id(workspace_id)
                        .await
                        .into_iter()
                        .collect()
                } else {
                    self.node_manager.get_online_nodes().await
                };
                candidates.retain(|node| {
                    if !senders.contains_key(&node.node_id) {
                        return false;
                    }

                    if node.state != crate::node_manager::NodeState::Online {
                        return false;
                    }

                    if let Some(required) = task.required_capabilities.as_ref() {
                        if !node.capabilities.meets(required) {
                            return false;
                        }
                    }

                    if !task.required_tags.is_empty()
                        && !task.required_tags.iter().any(|tag| node.tags.contains(tag))
                    {
                        return false;
                    }

                    if let Some(workspace_id) = execution_workspace_id.as_deref() {
                        if node.workspace.workspace_id.as_deref() != Some(workspace_id) {
                            return false;
                        }
                    } else if let Some(hint) = task.workspace_hint.as_deref() {
                        if !workspace_matches_hint(&node.workspace.path, hint) {
                            return false;
                        }
                    }

                    true
                });

                candidates.sort_by(|a, b| {
                    a.load
                        .score()
                        .partial_cmp(&b.load.score())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut task = Some(task);

                for node in candidates {
                    let assignment = {
                        let task_ref = task.as_ref().expect("task should exist before scheduling");
                        HubToNode::TaskAssignment {
                            message_id: MessageId::new(),
                            task_id: task_ref.task_id.clone(),
                            command: task_ref.command.clone(),
                            priority: task_ref.priority,
                            deadline: Some(
                                Utc::now()
                                    + chrono::Duration::seconds(self.task_timeout_secs as i64),
                            ),
                            context: uhorse_protocol::TaskContext {
                                user_id: task_ref.context.user_id.clone(),
                                session_id: task_ref.context.session_id.clone(),
                                channel: task_ref.context.channel.clone(),
                                execution_workspace_id: task_ref
                                    .context
                                    .execution_workspace_id
                                    .clone(),
                                collaboration_workspace_id: task_ref
                                    .context
                                    .collaboration_workspace_id
                                    .clone(),
                                intent: task_ref.context.intent.clone(),
                                env: task_ref.context.env.clone(),
                                created_at: task_ref.context.created_at,
                            },
                            retry_count: task_ref.retry_count,
                            max_retries: task_ref.max_retries,
                        }
                    };

                    let sender = senders
                        .get(&node.node_id)
                        .expect("candidate nodes must have active senders");

                    if let Err(e) = self
                        .node_manager
                        .send_to_node(&node.node_id, assignment, sender)
                        .await
                    {
                        warn!("Failed to send task to node {}: {}", node.node_id, e);
                        continue;
                    }

                    let task = task.take().expect("task should only be scheduled once");
                    let running = RunningTask {
                        task_id: task.task_id.clone(),
                        command: task.command,
                        context: task.context,
                        priority: task.priority,
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
                }

                let mut task = task.expect("task should be returned to queue when not scheduled");
                if task.retry_count < task.max_retries && !senders.is_empty() {
                    task.retry_count += 1;
                }

                let mut pending = self.pending_tasks.write().await;
                if let Some(queue) = pending.get_mut(&priority) {
                    queue.insert(0, task);
                }
                debug!("No suitable node available for task");
            }
        }

        Ok(None)
    }

    /// 处理任务完成
    pub async fn complete_task(
        &self,
        task_id: &TaskId,
        node_id: &NodeId,
        result: CommandResult,
    ) -> HubResult<()> {
        let mut running_tasks = self.running_tasks.write().await;

        if let Some(running) = running_tasks.remove(task_id) {
            let completed = CompletedTask {
                task_id: task_id.clone(),
                command: running.command,
                context: running.context,
                priority: running.priority,
                node_id: running.node_id.clone(),
                started_at: running.started_at,
                completed_at: Utc::now(),
                status: Self::terminal_status_from_result(&result),
                result: result.clone(),
            };
            drop(running_tasks);
            self.store_completed_task(completed).await;

            let task_result = TaskResult {
                task_id: task_id.clone(),
                node_id: node_id.clone(),
                result: result.clone(),
            };

            if self.result_tx.send(task_result).await.is_err() {
                warn!("Failed to send task result notification");
            }

            info!(
                "Task {} completed on node {}: {}",
                task_id,
                node_id,
                if result.success { "success" } else { "failed" }
            );
            return Ok(());
        }
        drop(running_tasks);

        if let Some(existing) = self.completed_tasks.read().await.get(task_id).cloned() {
            warn!(
                "Ignore late result for task {} from node {} because terminal status is already {:?}",
                task_id, node_id, existing.status
            );
            return Ok(());
        }

        warn!(
            "Ignore result for unknown task {} from node {} because task is no longer tracked",
            task_id, node_id
        );
        Ok(())
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &TaskId) -> HubResult<()> {
        {
            let mut pending = self.pending_tasks.write().await;
            for queue in pending.values_mut() {
                if let Some(pos) = queue.iter().position(|t| &t.task_id == task_id) {
                    let queued = queue.remove(pos);
                    drop(pending);

                    let result =
                        CommandResult::failure(ExecutionError::execution_failed("Task cancelled"));
                    let completed = CompletedTask {
                        task_id: queued.task_id.clone(),
                        command: queued.command,
                        context: queued.context,
                        priority: queued.priority,
                        node_id: NodeId::from_string("pending-cancelled"),
                        started_at: queued.created_at,
                        completed_at: Utc::now(),
                        status: TaskStatus::Cancelled,
                        result: result.clone(),
                    };
                    self.store_completed_task(completed).await;

                    if self
                        .result_tx
                        .send(TaskResult {
                            task_id: task_id.clone(),
                            node_id: NodeId::from_string("pending-cancelled"),
                            result,
                        })
                        .await
                        .is_err()
                    {
                        warn!("Failed to send queued cancelled task result notification");
                    }

                    info!("Task {} cancelled while queued", task_id);
                    return Ok(());
                }
            }
        }

        let running = {
            let mut running_tasks = self.running_tasks.write().await;
            running_tasks.remove(task_id)
        };

        if let Some(running) = running {
            let result = CommandResult::failure(ExecutionError::execution_failed("Task cancelled"));
            let completed = CompletedTask {
                task_id: task_id.clone(),
                command: running.command,
                context: running.context,
                priority: running.priority,
                node_id: running.node_id.clone(),
                started_at: running.started_at,
                completed_at: Utc::now(),
                status: TaskStatus::Cancelled,
                result: result.clone(),
            };
            self.store_completed_task(completed).await;

            if self
                .result_tx
                .send(TaskResult {
                    task_id: task_id.clone(),
                    node_id: running.node_id,
                    result,
                })
                .await
                .is_err()
            {
                warn!("Failed to send cancelled task result notification");
            }

            info!("Task {} cancelled while running", task_id);
            return Ok(());
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

                let result = CommandResult::failure(ExecutionError::timeout("Task timed out"));
                let completed = CompletedTask {
                    task_id: task_id.clone(),
                    command: running.command,
                    context: running.context,
                    priority: running.priority,
                    node_id: running.node_id.clone(),
                    started_at: running.started_at,
                    completed_at: now,
                    status: TaskStatus::Timeout,
                    result: result.clone(),
                };
                drop(running_tasks);
                self.store_completed_task(completed).await;

                if self
                    .result_tx
                    .send(TaskResult {
                        task_id: task_id.clone(),
                        node_id: running.node_id,
                        result,
                    })
                    .await
                    .is_err()
                {
                    warn!("Failed to send timeout task result notification");
                }

                running_tasks = self.running_tasks.write().await;
            }
        }

        timed_out
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &TaskId) -> Option<TaskStatusInfo> {
        {
            let running_tasks = self.running_tasks.read().await;
            if let Some(running) = running_tasks.get(task_id) {
                return Some(TaskStatusInfo {
                    task_id: task_id.clone(),
                    status: TaskStatus::Running,
                    command_type: Some(running.command.command_type()),
                    priority: Some(running.priority),
                    node_id: Some(running.node_id.clone()),
                    started_at: Some(running.started_at),
                    completed_at: None,
                    error: None,
                });
            }
        }

        {
            let completed_tasks = self.completed_tasks.read().await;
            if let Some(completed) = completed_tasks.get(task_id) {
                return Some(TaskStatusInfo {
                    task_id: task_id.clone(),
                    status: completed.status.clone(),
                    command_type: Some(completed.command.command_type()),
                    priority: Some(completed.priority),
                    node_id: Some(completed.node_id.clone()),
                    started_at: Some(completed.started_at),
                    completed_at: Some(completed.completed_at),
                    error: completed
                        .result
                        .error
                        .as_ref()
                        .map(|error| error.message.clone()),
                });
            }
        }

        {
            let pending = self.pending_tasks.read().await;
            for queue in pending.values() {
                if let Some(task) = queue.iter().find(|t| &t.task_id == task_id) {
                    return Some(TaskStatusInfo {
                        task_id: task_id.clone(),
                        status: TaskStatus::Queued,
                        command_type: Some(task.command.command_type()),
                        priority: Some(task.priority),
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

    /// 仅测试使用：插入已完成任务。
    #[cfg(test)]
    pub async fn insert_completed_task_for_test(&self, completed: CompletedTask) {
        self.completed_tasks
            .write()
            .await
            .insert(completed.task_id.clone(), completed);
    }

    /// 仅测试使用：插入运行中任务。
    #[cfg(test)]
    pub async fn insert_running_task_for_test(&self, running: RunningTask) {
        self.running_tasks
            .write()
            .await
            .insert(running.task_id.clone(), running);
    }

    /// 获取任务上下文
    pub async fn get_task_context(&self, task_id: &TaskId) -> Option<TaskContext> {
        {
            let running_tasks = self.running_tasks.read().await;
            if let Some(running) = running_tasks.get(task_id) {
                return Some(running.context.clone());
            }
        }

        {
            let completed_tasks = self.completed_tasks.read().await;
            if let Some(completed) = completed_tasks.get(task_id) {
                return Some(completed.context.clone());
            }
        }

        {
            let pending = self.pending_tasks.read().await;
            for queue in pending.values() {
                if let Some(task) = queue.iter().find(|task| &task.task_id == task_id) {
                    return Some(task.context.clone());
                }
            }
        }

        None
    }

    /// 获取已完成任务详情
    pub async fn get_completed_task(&self, task_id: &TaskId) -> Option<CompletedTask> {
        self.completed_tasks.read().await.get(task_id).cloned()
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
            match task.status {
                TaskStatus::Completed => completed_count += 1,
                TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Timeout => failed_count += 1,
                _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_manager::NodeManager;
    use uhorse_protocol::UserId;

    #[tokio::test]
    async fn test_cancel_running_task_sets_cancelled_status() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, mut rx) = TaskScheduler::new(node_manager, 3, 300);
        let task_id = TaskId::from_string("task-cancel-status");
        scheduler
            .insert_running_task_for_test(RunningTask {
                task_id: task_id.clone(),
                command: Command::File(uhorse_protocol::FileCommand::Exists {
                    path: "README.md".to_string(),
                }),
                context: TaskContext::new(
                    UserId::from_string("user-1"),
                    uhorse_protocol::SessionId::from_string("session-1"),
                    "dingtalk",
                ),
                priority: Priority::Normal,
                node_id: NodeId::from_string("node-1"),
                started_at: Utc::now(),
                timeout_at: Utc::now() + chrono::Duration::seconds(30),
                retry_count: 0,
            })
            .await;

        scheduler.cancel_task(&task_id).await.unwrap();

        let status = scheduler.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Cancelled);
        assert_eq!(status.error.as_deref(), Some("Task cancelled"));
        let result = rx.recv().await.unwrap();
        assert_eq!(result.task_id, task_id);
        assert!(!result.result.success);
    }

    #[tokio::test]
    async fn test_cancel_queued_task_sets_cancelled_status() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, mut rx) = TaskScheduler::new(node_manager, 3, 300);
        let task_id = scheduler
            .submit_task(
                Command::File(uhorse_protocol::FileCommand::Exists {
                    path: "README.md".to_string(),
                }),
                TaskContext::new(
                    UserId::from_string("user-1"),
                    uhorse_protocol::SessionId::from_string("session-1"),
                    "dingtalk",
                ),
                Priority::Normal,
                None,
                vec![],
                None,
            )
            .await
            .unwrap();

        scheduler.cancel_task(&task_id).await.unwrap();

        let status = scheduler.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Cancelled);
        assert_eq!(status.error.as_deref(), Some("Task cancelled"));
        let result = rx.recv().await.unwrap();
        assert_eq!(result.task_id, task_id);
        assert!(!result.result.success);
    }

    #[tokio::test]
    async fn test_timeout_task_sets_timeout_status() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, mut rx) = TaskScheduler::new(node_manager, 3, 300);
        let task_id = TaskId::from_string("task-timeout-status");
        scheduler
            .insert_running_task_for_test(RunningTask {
                task_id: task_id.clone(),
                command: Command::File(uhorse_protocol::FileCommand::Exists {
                    path: "README.md".to_string(),
                }),
                context: TaskContext::new(
                    UserId::from_string("user-1"),
                    uhorse_protocol::SessionId::from_string("session-1"),
                    "dingtalk",
                ),
                priority: Priority::Normal,
                node_id: NodeId::from_string("node-1"),
                started_at: Utc::now() - chrono::Duration::seconds(60),
                timeout_at: Utc::now() - chrono::Duration::seconds(1),
                retry_count: 0,
            })
            .await;

        let timed_out = scheduler.check_timeouts().await;
        assert_eq!(timed_out, vec![task_id.clone()]);
        let status = scheduler.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Timeout);
        assert_eq!(status.error.as_deref(), Some("Task timed out"));
        let result = rx.recv().await.unwrap();
        assert_eq!(result.task_id, task_id);
        assert!(!result.result.success);
    }

    #[tokio::test]
    async fn test_late_result_does_not_override_terminal_status() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, mut rx) = TaskScheduler::new(node_manager, 3, 300);
        let task_id = TaskId::from_string("task-late-result");
        let node_id = NodeId::from_string("node-1");
        scheduler
            .insert_running_task_for_test(RunningTask {
                task_id: task_id.clone(),
                command: Command::File(uhorse_protocol::FileCommand::Exists {
                    path: "README.md".to_string(),
                }),
                context: TaskContext::new(
                    UserId::from_string("user-1"),
                    uhorse_protocol::SessionId::from_string("session-1"),
                    "dingtalk",
                ),
                priority: Priority::Normal,
                node_id: node_id.clone(),
                started_at: Utc::now(),
                timeout_at: Utc::now() + chrono::Duration::seconds(30),
                retry_count: 0,
            })
            .await;

        scheduler.cancel_task(&task_id).await.unwrap();
        let first_result = rx.recv().await.unwrap();
        assert_eq!(first_result.task_id, task_id);

        scheduler
            .complete_task(
                &task_id,
                &node_id,
                CommandResult::success(uhorse_protocol::CommandOutput::text("late success")),
            )
            .await
            .unwrap();

        let status = scheduler.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Cancelled);
        assert_eq!(status.error.as_deref(), Some("Task cancelled"));
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
            .await
            .is_err());
    }
}
