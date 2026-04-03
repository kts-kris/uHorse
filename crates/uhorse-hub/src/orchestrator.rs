//! 智能编排器
//!
//! 复用 uhorse-agent 的能力实现 Hub 级别的智能编排：
//! - 意图理解
//! - 任务规划
//! - 结果汇总

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use uhorse_agent::SkillRegistry;
use uhorse_protocol::{Command, NodeId, Priority, ShellCommand, TaskContext, TaskId};

use crate::error::{HubError, HubResult};
use crate::node_manager::NodeManager;
use crate::task_scheduler::TaskScheduler;

/// 编排计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    /// 计划 ID
    pub plan_id: String,
    /// 原始意图
    pub intent: String,
    /// 解析后的子任务
    pub subtasks: Vec<SubTask>,
    /// 预期输出
    pub expected_output: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 子任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    /// 子任务 ID
    pub id: String,
    /// 命令
    pub command: Command,
    /// 依赖的子任务
    pub dependencies: Vec<String>,
    /// 优先级
    pub priority: Priority,
    /// 所需能力
    pub required_capabilities: Option<uhorse_protocol::NodeCapabilities>,
    /// 所需标签
    pub required_tags: Vec<String>,
}

/// 编排结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationResult {
    /// 计划 ID
    pub plan_id: String,
    /// 是否成功
    pub success: bool,
    /// 子任务结果
    pub subtask_results: Vec<SubTaskResult>,
    /// 汇总结果
    pub summary: String,
    /// 完成时间
    pub completed_at: DateTime<Utc>,
}

/// 子任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTaskResult {
    /// 子任务 ID
    pub id: String,
    /// 任务 ID
    pub task_id: TaskId,
    /// 执行节点
    pub node_id: NodeId,
    /// 是否成功
    pub success: bool,
    /// 输出
    pub output: String,
    /// 错误
    pub error: Option<String>,
}

/// 智能编排器
///
/// 复用 uhorse-agent 的能力实现智能编排
pub struct Orchestrator {
    /// 技能注册表
    skill_registry: Arc<SkillRegistry>,
    /// 任务调度器
    task_scheduler: Arc<TaskScheduler>,
    /// 节点管理器
    _node_manager: Arc<NodeManager>,
}

impl Orchestrator {
    /// 创建新的编排器
    pub fn new(
        skill_registry: Arc<SkillRegistry>,
        task_scheduler: Arc<TaskScheduler>,
        node_manager: Arc<NodeManager>,
    ) -> Self {
        Self {
            skill_registry,
            task_scheduler,
            _node_manager: node_manager,
        }
    }

    /// 创建默认编排器
    pub fn create(task_scheduler: Arc<TaskScheduler>, node_manager: Arc<NodeManager>) -> Self {
        let skill_registry = Arc::new(SkillRegistry::new());
        Self::new(skill_registry, task_scheduler, node_manager)
    }

    /// 理解用户意图并生成编排计划
    pub async fn understand_intent(
        &self,
        user_input: &str,
        context: &TaskContext,
    ) -> HubResult<OrchestrationPlan> {
        let plan_id = format!("plan-{}", uuid::Uuid::new_v4());

        // 简化的意图理解：基于关键词识别
        let subtasks = self.parse_intent_to_subtasks(user_input, context)?;

        info!(
            "Orchestration plan {} created with {} subtasks",
            plan_id,
            subtasks.len()
        );

        Ok(OrchestrationPlan {
            plan_id,
            intent: user_input.to_string(),
            subtasks,
            expected_output: "Task completed".to_string(),
            created_at: Utc::now(),
        })
    }

    /// 解析意图为子任务
    fn parse_intent_to_subtasks(
        &self,
        input: &str,
        _context: &TaskContext,
    ) -> HubResult<Vec<SubTask>> {
        let mut subtasks = Vec::new();

        // 基于关键词识别命令类型
        let lower_input = input.to_lowercase();

        if lower_input.contains("run")
            || lower_input.contains("execute")
            || lower_input.contains("执行")
        {
            // Shell 命令
            subtasks.push(SubTask {
                id: "subtask-0".to_string(),
                command: Command::Shell(ShellCommand {
                    command: input.to_string(),
                    args: vec![],
                    env: Default::default(),
                    cwd: None,
                    timeout: std::time::Duration::from_secs(300),
                    capture_stderr: true,
                }),
                dependencies: vec![],
                priority: Priority::Normal,
                required_capabilities: None,
                required_tags: vec![],
            });
        } else {
            // 默认为 Shell 命令
            subtasks.push(SubTask {
                id: "subtask-0".to_string(),
                command: Command::Shell(ShellCommand {
                    command: input.to_string(),
                    args: vec![],
                    env: Default::default(),
                    cwd: None,
                    timeout: std::time::Duration::from_secs(300),
                    capture_stderr: true,
                }),
                dependencies: vec![],
                priority: Priority::Normal,
                required_capabilities: None,
                required_tags: vec![],
            });
        }

        Ok(subtasks)
    }

    /// 执行编排计划
    pub async fn execute_plan(
        &self,
        plan: &OrchestrationPlan,
        context: TaskContext,
    ) -> HubResult<OrchestrationResult> {
        let mut subtask_results = Vec::new();
        let mut completed_ids = std::collections::HashSet::new();

        // 按依赖关系排序执行
        let mut remaining: Vec<_> = plan.subtasks.iter().collect();

        while !remaining.is_empty() {
            let mut progress = false;

            // 找出所有依赖已满足的子任务
            let ready: Vec<_> = remaining
                .iter()
                .filter(|st| {
                    st.dependencies
                        .iter()
                        .all(|dep| completed_ids.contains(dep))
                })
                .collect();

            if ready.is_empty() && !remaining.is_empty() {
                warn!(
                    "Circular dependency or unmet dependencies in plan {}",
                    plan.plan_id
                );
                break;
            }

            // 并行提交就绪的子任务
            let mut task_ids = Vec::new();
            for subtask in &ready {
                let task_id = self
                    .task_scheduler
                    .submit_task(
                        subtask.command.clone(),
                        context.clone(),
                        subtask.priority,
                        subtask.required_capabilities.clone(),
                        subtask.required_tags.clone(),
                        None,
                    )
                    .await?;

                task_ids.push((subtask.id.clone(), task_id));
                progress = true;
            }

            // 等待任务完成并收集结果
            for (subtask_id, task_id) in task_ids {
                let result = self.wait_for_task(&task_id).await?;

                completed_ids.insert(subtask_id.clone());
                subtask_results.push(SubTaskResult {
                    id: subtask_id,
                    task_id,
                    node_id: result.node_id,
                    success: result.success,
                    output: result.output.unwrap_or_default(),
                    error: result.error,
                });
            }

            // 移除已完成的任务
            remaining.retain(|st| !completed_ids.contains(&st.id));

            if !progress {
                break;
            }
        }

        // 汇总结果
        let summary = self.summarize_results(&subtask_results);

        Ok(OrchestrationResult {
            plan_id: plan.plan_id.clone(),
            success: subtask_results.iter().all(|r| r.success),
            subtask_results,
            summary,
            completed_at: Utc::now(),
        })
    }

    /// 等待任务完成
    async fn wait_for_task(&self, task_id: &TaskId) -> HubResult<TaskCompletion> {
        // 简单轮询实现
        for _ in 0..100 {
            if let Some(status) = self.task_scheduler.get_task_status(task_id).await {
                match status.status {
                    uhorse_protocol::TaskStatus::Completed => {
                        return Ok(TaskCompletion {
                            task_id: task_id.clone(),
                            node_id: status
                                .node_id
                                .unwrap_or_else(|| NodeId::from_string("unknown")),
                            success: true,
                            output: None,
                            error: status.error,
                        });
                    }
                    uhorse_protocol::TaskStatus::Failed
                    | uhorse_protocol::TaskStatus::Cancelled
                    | uhorse_protocol::TaskStatus::Timeout => {
                        return Ok(TaskCompletion {
                            task_id: task_id.clone(),
                            node_id: status
                                .node_id
                                .unwrap_or_else(|| NodeId::from_string("unknown")),
                            success: false,
                            output: None,
                            error: status.error,
                        });
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Err(HubError::Timeout(format!(
            "Task {} did not complete in time",
            task_id
        )))
    }

    /// 汇总结果
    fn summarize_results(&self, results: &[SubTaskResult]) -> String {
        let total = results.len();
        let successful = results.iter().filter(|r| r.success).count();

        if successful == total {
            format!("All {} tasks completed successfully", total)
        } else {
            format!("{}/{} tasks completed successfully", successful, total)
        }
    }

    /// 获取技能注册表
    pub fn skill_registry(&self) -> Arc<SkillRegistry> {
        self.skill_registry.clone()
    }
}

/// 任务完成结果
#[allow(dead_code)]
struct TaskCompletion {
    task_id: TaskId,
    node_id: NodeId,
    success: bool,
    output: Option<String>,
    error: Option<String>,
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("task_scheduler", &"TaskScheduler")
            .field("node_manager", &"NodeManager")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uhorse_protocol::{CommandResult, ExecutionError, FileCommand, UserId};

    #[tokio::test]
    async fn test_wait_for_task_treats_cancelled_as_terminal() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, _rx) = TaskScheduler::new(node_manager.clone(), 3, 300);
        let scheduler = Arc::new(scheduler);
        let orchestrator = Orchestrator::create(scheduler.clone(), node_manager);
        let task_id = TaskId::from_string("task-orch-cancelled");

        scheduler
            .insert_completed_task_for_test(crate::task_scheduler::CompletedTask {
                task_id: task_id.clone(),
                command: Command::File(FileCommand::Exists {
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
                completed_at: Utc::now(),
                status: uhorse_protocol::TaskStatus::Cancelled,
                result: CommandResult::failure(ExecutionError::execution_failed("Task cancelled")),
            })
            .await;

        let result = orchestrator.wait_for_task(&task_id).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("Task cancelled"));
    }

    #[tokio::test]
    async fn test_wait_for_task_treats_timeout_as_terminal() {
        let node_manager = Arc::new(NodeManager::new(8, 30));
        let (scheduler, _rx) = TaskScheduler::new(node_manager.clone(), 3, 300);
        let scheduler = Arc::new(scheduler);
        let orchestrator = Orchestrator::create(scheduler.clone(), node_manager);
        let task_id = TaskId::from_string("task-orch-timeout");

        scheduler
            .insert_completed_task_for_test(crate::task_scheduler::CompletedTask {
                task_id: task_id.clone(),
                command: Command::File(FileCommand::Exists {
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
                completed_at: Utc::now(),
                status: uhorse_protocol::TaskStatus::Timeout,
                result: CommandResult::failure(ExecutionError::timeout("Task timed out")),
            })
            .await;

        let result = orchestrator.wait_for_task(&task_id).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("Task timed out"));
    }
}
