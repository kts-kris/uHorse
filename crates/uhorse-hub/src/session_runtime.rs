//! Session runtime 串行执行骨架。
//!
//! Phase 1 先提供按 session 串行化的最小能力，后续再扩展为完整的
//! per-session actor mailbox / ReAct loop runtime。

use crate::web::DingTalkReplyRoute;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::debug;
use uhorse_protocol::TaskId;
use uuid::Uuid;

/// Transcript 事件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranscriptEventKind {
    /// 用户消息进入 turn。
    UserMessage,
    /// Assistant 做出一步决策。
    AssistantStep,
    /// Tool 调用已规划。
    ToolCallPlanned,
    /// Tool 调用已派发到 backend task。
    ToolCallDispatched,
    /// Tool 调用进入审批等待。
    ApprovalRequested,
    /// Tool 结果已回流为 observation。
    ToolResultObserved,
    /// 审批已通过。
    ApprovalApproved,
    /// 审批已拒绝。
    ApprovalRejected,
    /// Turn 已从等待态恢复继续执行。
    TurnResumed,
    /// Planner 发生重试。
    PlannerRetry,
    /// Assistant 最终回复已生成。
    AssistantFinal,
    /// Turn 失败。
    TurnFailed,
    /// Turn 被取消。
    TurnCancelled,
    /// Turn 做过 compact。
    TurnCompacted,
}

/// 单条 transcript 事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEvent {
    /// 事件序号。
    pub seq: u64,
    /// 事件时间。
    pub created_at: DateTime<Utc>,
    /// 事件类型。
    pub kind: TranscriptEventKind,
    /// 事件内容。
    pub content: String,
}

/// 单个 turn 的 transcript。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionTranscript {
    /// 所属 turn ID。
    pub turn_id: String,
    /// transcript 事件列表。
    pub events: Vec<TranscriptEvent>,
}

/// Turn 执行状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnStatus {
    /// Turn 已创建，正在执行规划或直答。
    Running,
    /// Turn 已派发 tool，等待结果回流。
    WaitingForTool,
    /// Turn 已从等待态恢复，正在继续执行。
    Resuming,
    /// Turn 已完成。
    Completed,
    /// Turn 已取消。
    Cancelled,
    /// Turn 已失败。
    Failed,
}

/// Tool call 执行状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallStatus {
    /// Tool call 已发起，等待完成。
    Running,
    /// Tool call 已完成。
    Completed,
    /// Tool call 已失败。
    Failed,
}

/// Session 内当前活跃 turn。
#[derive(Debug, Clone)]
pub struct SessionTurnState {
    /// 当前 turn ID。
    pub turn_id: String,
    /// 用户原始消息。
    pub user_message: String,
    /// 当前 turn 状态。
    pub status: TurnStatus,
    /// 当前 step 序号。
    pub step_count: u32,
    /// 单个 turn 允许的最大 step 数。
    pub max_steps: u32,
    /// 是否已请求取消。
    pub cancel_requested: bool,
    /// 当前 step planner 已重试次数。
    pub planner_retry_count: u32,
    /// 当前 step planner 最大重试次数。
    pub max_planner_retries: u32,
    /// 已压缩的历史摘要。
    pub compacted_summary: Option<String>,
    /// 已被摘要覆盖的事件数量。
    pub pruned_event_count: usize,
    /// 当前活跃 tool call。
    pub tool_call: Option<ToolCallState>,
}

/// 最小 tool call 状态。
#[derive(Debug, Clone)]
pub struct ToolCallState {
    /// Tool call ID。
    pub tool_call_id: String,
    /// Tool 名称。
    pub tool_name: String,
    /// 关联的 Hub task ID。
    pub task_id: Option<TaskId>,
    /// Tool call 当前状态。
    pub status: ToolCallStatus,
}

/// 任务结果回流到 session actor 所需的关联信息。
#[derive(Debug, Clone)]
pub struct TaskContinuationBinding {
    /// 归属 session key。
    pub session_key: String,
    /// 所属 turn ID。
    pub turn_id: String,
    /// 对应 tool call ID。
    pub tool_call_id: String,
    /// 发起该 turn 的 agent ID。
    pub agent_id: String,
    /// 结果回写路由。
    pub route: DingTalkReplyRoute,
}

/// Session runtime 管理器。
#[derive(Debug, Default)]
pub struct SessionRuntimeManager {
    lanes: RwLock<HashMap<String, Arc<Mutex<()>>>>,
    turns: RwLock<HashMap<String, SessionTurnState>>,
    transcripts: RwLock<HashMap<String, SessionTranscript>>,
    task_bindings: RwLock<HashMap<TaskId, TaskContinuationBinding>>,
}

impl SessionRuntimeManager {
    /// 创建新的 session runtime 管理器。
    pub fn new() -> Self {
        Self::default()
    }

    async fn lane_for(&self, session_key: &str) -> Arc<Mutex<()>> {
        if let Some(lane) = self.lanes.read().await.get(session_key).cloned() {
            return lane;
        }

        let mut lanes = self.lanes.write().await;
        lanes
            .entry(session_key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// 开始一个新的 turn。
    pub async fn start_turn(&self, session_key: &str, user_message: impl Into<String>) -> String {
        let turn_id = Uuid::new_v4().to_string();
        let user_message = user_message.into();
        let turn = SessionTurnState {
            turn_id: turn_id.clone(),
            user_message: user_message.clone(),
            status: TurnStatus::Running,
            step_count: 0,
            max_steps: 4,
            cancel_requested: false,
            planner_retry_count: 0,
            max_planner_retries: 1,
            compacted_summary: None,
            pruned_event_count: 0,
            tool_call: None,
        };
        self.turns
            .write()
            .await
            .insert(session_key.to_string(), turn);
        self.transcripts.write().await.insert(
            session_key.to_string(),
            SessionTranscript {
                turn_id: turn_id.clone(),
                events: vec![TranscriptEvent {
                    seq: 1,
                    created_at: Utc::now(),
                    kind: TranscriptEventKind::UserMessage,
                    content: user_message,
                }],
            },
        );
        turn_id
    }

    /// 将当前 turn 标记为等待 tool 结果。
    pub async fn begin_tool_call(
        &self,
        session_key: &str,
        tool_name: impl Into<String>,
    ) -> Option<String> {
        let mut turns = self.turns.write().await;
        let turn = turns.get_mut(session_key)?;
        let tool_call_id = Uuid::new_v4().to_string();
        turn.status = TurnStatus::WaitingForTool;
        turn.tool_call = Some(ToolCallState {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.into(),
            task_id: None,
            status: ToolCallStatus::Running,
        });
        Some(tool_call_id)
    }

    /// 追加 transcript 事件。
    pub async fn append_transcript_event(
        &self,
        session_key: &str,
        kind: TranscriptEventKind,
        content: impl Into<String>,
    ) {
        let mut transcripts = self.transcripts.write().await;
        let transcript = transcripts.entry(session_key.to_string()).or_insert_with(|| {
            SessionTranscript {
                turn_id: String::new(),
                events: Vec::new(),
            }
        });
        let seq = transcript.events.len() as u64 + 1;
        transcript.events.push(TranscriptEvent {
            seq,
            created_at: Utc::now(),
            kind,
            content: content.into(),
        });
    }

    /// 获取当前 session transcript。
    pub async fn transcript(&self, session_key: &str) -> Option<SessionTranscript> {
        self.transcripts.read().await.get(session_key).cloned()
    }

    /// 将任务与当前 tool call 绑定。
    pub async fn bind_task_to_turn(&self, task_id: TaskId, binding: TaskContinuationBinding) {
        let session_key = binding.session_key.clone();
        let tool_call_id = binding.tool_call_id.clone();
        self.task_bindings.write().await.insert(task_id.clone(), binding);

        if let Some(turn) = self.turns.write().await.get_mut(&session_key) {
            if let Some(tool_call) = turn.tool_call.as_mut() {
                if tool_call.tool_call_id == tool_call_id {
                    tool_call.task_id = Some(task_id.clone());
                }
            }
        }

        self.append_transcript_event(
            &session_key,
            TranscriptEventKind::ToolCallDispatched,
            format!("{}:{}", tool_call_id, task_id),
        )
        .await;
    }

    /// 获取任务 continuation 绑定。
    pub async fn task_binding(&self, task_id: &TaskId) -> Option<TaskContinuationBinding> {
        self.task_bindings.read().await.get(task_id).cloned()
    }

    /// 移除任务 continuation 绑定。
    pub async fn take_task_binding(&self, task_id: &TaskId) -> Option<TaskContinuationBinding> {
        self.task_bindings.write().await.remove(task_id)
    }

    /// 将当前 turn 标记为完成。
    pub async fn complete_turn(&self, session_key: &str, task_id: Option<&TaskId>) {
        if let Some(turn) = self.turns.write().await.get_mut(session_key) {
            turn.status = TurnStatus::Completed;
            if let Some(tool_call) = turn.tool_call.as_mut() {
                if task_id.is_none() || tool_call.task_id.as_ref() == task_id {
                    tool_call.status = ToolCallStatus::Completed;
                }
            }
        }
        if let Some(task_id) = task_id {
            self.task_bindings.write().await.remove(task_id);
        }
    }

    /// 将当前 turn 标记为取消。
    pub async fn cancel_turn(&self, session_key: &str) {
        if let Some(turn) = self.turns.write().await.get_mut(session_key) {
            turn.cancel_requested = true;
            turn.status = TurnStatus::Cancelled;
            if let Some(tool_call) = turn.tool_call.as_mut() {
                tool_call.status = ToolCallStatus::Failed;
            }
        }
        self.append_transcript_event(session_key, TranscriptEventKind::TurnCancelled, "cancelled")
            .await;
    }

    /// 将当前 turn 标记为失败。
    pub async fn fail_turn(&self, session_key: &str, task_id: Option<&TaskId>) {
        if let Some(turn) = self.turns.write().await.get_mut(session_key) {
            turn.status = TurnStatus::Failed;
            if let Some(tool_call) = turn.tool_call.as_mut() {
                if task_id.is_none() || tool_call.task_id.as_ref() == task_id {
                    tool_call.status = ToolCallStatus::Failed;
                }
            }
        }
        if let Some(task_id) = task_id {
            self.task_bindings.write().await.remove(task_id);
        }
    }

    /// 当前 turn step 数加一，并返回最新状态。
    pub async fn increment_step_count(&self, session_key: &str) -> Option<SessionTurnState> {
        let mut turns = self.turns.write().await;
        let turn = turns.get_mut(session_key)?;
        turn.step_count += 1;
        Some(turn.clone())
    }

    /// 获取 session 当前 turn 状态。
    pub async fn turn_state(&self, session_key: &str) -> Option<SessionTurnState> {
        self.turns.read().await.get(session_key).cloned()
    }

    /// 将当前 turn 标记为恢复继续执行。
    pub async fn mark_turn_resuming(
        &self,
        session_key: &str,
        task_id: Option<&TaskId>,
    ) -> Option<SessionTurnState> {
        let mut turns = self.turns.write().await;
        let turn = turns.get_mut(session_key)?;
        if let Some(tool_call) = turn.tool_call.as_mut() {
            if task_id.is_none() || tool_call.task_id.as_ref() == task_id {
                tool_call.status = ToolCallStatus::Completed;
            }
        }
        turn.status = TurnStatus::Resuming;
        Some(turn.clone())
    }

    /// 获取指定 task 对应的 continuation 绑定。
    pub async fn find_task_binding(&self, task_id: &TaskId) -> Option<TaskContinuationBinding> {
        self.task_bindings.read().await.get(task_id).cloned()
    }

    /// 记录 planner 重试次数。
    pub async fn increment_planner_retry(&self, session_key: &str) -> Option<SessionTurnState> {
        let mut turns = self.turns.write().await;
        let turn = turns.get_mut(session_key)?;
        turn.planner_retry_count += 1;
        Some(turn.clone())
    }

    /// 重置 planner 重试次数。
    pub async fn reset_planner_retry(&self, session_key: &str) {
        if let Some(turn) = self.turns.write().await.get_mut(session_key) {
            turn.planner_retry_count = 0;
        }
    }

    /// 更新 turn 的逻辑压缩摘要。
    pub async fn record_compaction(
        &self,
        session_key: &str,
        summary: impl Into<String>,
        pruned_event_count: usize,
    ) {
        if let Some(turn) = self.turns.write().await.get_mut(session_key) {
            turn.compacted_summary = Some(summary.into());
            turn.pruned_event_count = pruned_event_count;
        }
    }

    /// 对已完成 turn 执行最小物理裁剪，仅保留关键事件与最近尾部。
    pub async fn prune_completed_turn_transcript(&self, session_key: &str, tail_len: usize) {
        let mut transcripts = self.transcripts.write().await;
        let Some(transcript) = transcripts.get_mut(session_key) else {
            return;
        };
        if transcript.events.len() <= tail_len {
            return;
        }

        let tail_start = transcript.events.len().saturating_sub(tail_len);
        let mut retained = Vec::new();
        for (index, event) in transcript.events.iter().enumerate() {
            let keep = matches!(
                event.kind,
                TranscriptEventKind::UserMessage
                    | TranscriptEventKind::AssistantFinal
                    | TranscriptEventKind::TurnCompacted
                    | TranscriptEventKind::TurnCancelled
                    | TranscriptEventKind::TurnFailed
            ) || index >= tail_start;
            if keep {
                retained.push(event.clone());
            }
        }

        for (seq, event) in retained.iter_mut().enumerate() {
            event.seq = seq as u64 + 1;
        }
        transcript.events = retained;
    }

    /// 按 session 串行执行一个异步任务。
    pub async fn run_serialized<Fut, T>(&self, session_key: &str, fut: Fut) -> T
    where
        Fut: Future<Output = T>,
    {
        let lane = self.lane_for(session_key).await;
        let _guard = lane.lock().await;
        debug!(session_key, "Running serialized session task");
        fut.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uhorse_protocol::TaskId;

    fn sample_route() -> DingTalkReplyRoute {
        DingTalkReplyRoute {
            conversation_id: "conversation-1".to_string(),
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: None,
        }
    }

    #[tokio::test]
    async fn test_session_runtime_manager_serializes_same_session_tasks() {
        let manager = Arc::new(SessionRuntimeManager::new());
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let first = {
            let manager = manager.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            tokio::spawn(async move {
                manager
                    .run_serialized("session-1", async move {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(current, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await;
            })
        };

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let second = {
            let manager = manager.clone();
            let active = active.clone();
            let max_active = max_active.clone();
            tokio::spawn(async move {
                manager
                    .run_serialized("session-1", async move {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(current, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await;
            })
        };

        first.await.unwrap();
        second.await.unwrap();

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_session_runtime_manager_tracks_turn_and_tool_call_state() {
        let manager = SessionRuntimeManager::new();
        let turn_id = manager.start_turn("session-1", "查看 README").await;
        let tool_call_id = manager
            .begin_tool_call("session-1", "hub_task")
            .await
            .unwrap();
        let task_id = TaskId::from_string("task-1");

        manager
            .bind_task_to_turn(
                task_id.clone(),
                TaskContinuationBinding {
                    session_key: "session-1".to_string(),
                    turn_id: turn_id.clone(),
                    tool_call_id: tool_call_id.clone(),
                    agent_id: "main".to_string(),
                    route: sample_route(),
                },
            )
            .await;

        let turn = manager.turn_state("session-1").await.unwrap();
        assert_eq!(turn.turn_id, turn_id);
        assert_eq!(turn.status, TurnStatus::WaitingForTool);
        assert!(!turn.cancel_requested);
        assert_eq!(turn.planner_retry_count, 0);
        assert_eq!(turn.max_planner_retries, 1);
        assert_eq!(turn.pruned_event_count, 0);
        assert!(turn.compacted_summary.is_none());
        let tool_call = turn.tool_call.unwrap();
        assert_eq!(tool_call.tool_call_id, tool_call_id);
        assert_eq!(tool_call.task_id.as_ref(), Some(&task_id));
        assert_eq!(tool_call.status, ToolCallStatus::Running);

        let binding = manager.task_binding(&task_id).await.unwrap();
        assert_eq!(binding.session_key, "session-1");
        assert_eq!(binding.turn_id, turn_id);

        let resumed = manager
            .mark_turn_resuming("session-1", Some(&task_id))
            .await
            .unwrap();
        assert_eq!(resumed.status, TurnStatus::Resuming);
        assert_eq!(
            resumed.tool_call.as_ref().map(|tool_call| tool_call.status.clone()),
            Some(ToolCallStatus::Completed)
        );
    }
}
