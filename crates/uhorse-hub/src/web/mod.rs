//! Web 管理界面
//!
//! 提供 Hub 的 HTTP 管理接口

pub mod ws;

use axum::{
    extract::{MatchedPath, Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use uhorse_agent::{
    Agent, AgentManager, AgentScope, AgentScopeConfig, FileMemory, MemoryStore, SessionKey,
    SessionState, SkillRegistry,
};
use uhorse_channel::{dingtalk::DingTalkEvent, DingTalkChannel, DingTalkInboundMessage};
use uhorse_core::{Channel, MessageContent, SessionId as CoreSessionId};
use uhorse_llm::{ChatMessage, LLMClient};
use uhorse_observability::{HealthService, HealthStatus, MetricsCollector, MetricsExporter};
use uhorse_protocol::{
    Command, CommandOutput, FileCommand, HubToNode, MessageId,
    PermissionRule as ProtocolPermissionRule, Priority, SessionId, TaskContext, TaskId, UserId,
};
use uhorse_security::ApprovalRequest;

use crate::{
    task_scheduler::{CompletedTask, TaskResult},
    Hub, HubStats,
};
pub use ws::ws_handler;

/// DingTalk 回传路由
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkReplyRoute {
    /// 会话 ID
    pub conversation_id: String,
    /// 会话类型
    pub conversation_type: Option<String>,
    /// 发送人 ID
    pub sender_user_id: Option<String>,
    /// 发送人工号
    pub sender_staff_id: Option<String>,
    /// 会话回调 Webhook
    pub session_webhook: Option<String>,
    /// 回调 Webhook 过期时间（毫秒时间戳）
    pub session_webhook_expired_time: Option<i64>,
    /// 机器人编码
    pub robot_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlannedDingTalkCommand {
    command: Command,
    #[serde(default)]
    workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentDecision {
    DirectReply { text: String },
    ExecuteCommand { command: Command },
    ExecuteSkill { skill_name: String, input: String },
}

#[derive(Debug, Clone, Serialize)]
struct AgentRuntimeSummary {
    agent_id: String,
    name: String,
    description: String,
    workspace_dir: String,
    is_default: bool,
    skill_names: Vec<String>,
    active_session_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AgentRuntimeDetail {
    agent_id: String,
    name: String,
    description: String,
    workspace_dir: String,
    system_prompt: String,
    is_default: bool,
    skill_names: Vec<String>,
    active_session_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SkillRuntimeSummary {
    name: String,
    description: String,
    version: String,
    enabled: bool,
    timeout_secs: u64,
    max_retries: usize,
    executable: Option<String>,
    args: Vec<String>,
    permissions: Vec<String>,
    execution_mode: String,
}

#[derive(Debug, Clone, Serialize)]
struct SkillRuntimeDetail {
    name: String,
    description: String,
    version: String,
    author: Option<String>,
    enabled: bool,
    timeout_secs: u64,
    max_retries: usize,
    executable: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    permissions: Vec<String>,
    execution_mode: String,
}

#[derive(Debug, Clone, Serialize)]
struct SessionRuntimeSummary {
    session_id: String,
    agent_id: Option<String>,
    conversation_id: Option<String>,
    sender_user_id: Option<String>,
    sender_staff_id: Option<String>,
    last_task_id: Option<String>,
    message_count: usize,
    created_at: String,
    last_active: String,
}

#[derive(Debug, Clone, Serialize)]
struct SessionRuntimeDetail {
    session_id: String,
    agent_id: Option<String>,
    conversation_id: Option<String>,
    sender_user_id: Option<String>,
    sender_staff_id: Option<String>,
    last_task_id: Option<String>,
    message_count: usize,
    created_at: String,
    last_active: String,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct SessionMessageRecord {
    timestamp: String,
    user_message: String,
    assistant_message: String,
}

/// Web Agent 运行时依赖集合
#[derive(Clone)]
pub struct WebAgentRuntime {
    /// Agent 管理器
    pub agent_manager: Arc<AgentManager>,
    /// 已注册 Agent 集合
    pub agents: Arc<HashMap<String, Agent>>,
    /// Memory 存储
    pub memory_store: Arc<dyn MemoryStore>,
    /// Skill 注册表
    pub skills: Arc<SkillRegistry>,
}

/// Web 服务器状态
#[derive(Clone)]
pub struct WebState {
    /// Hub 引用
    pub hub: Arc<Hub>,
    /// 健康检查服务
    pub health_service: Arc<HealthService>,
    /// Metrics 收集器
    pub metrics_collector: Arc<MetricsCollector>,
    /// Metrics 导出器
    pub metrics_exporter: Arc<MetricsExporter>,
    /// DingTalk 通道
    pub dingtalk_channel: Option<Arc<DingTalkChannel>>,
    /// LLM 客户端
    pub llm_client: Option<Arc<dyn LLMClient>>,
    /// Agent 运行时
    pub agent_runtime: Arc<WebAgentRuntime>,
    /// 任务回传路由
    pub dingtalk_routes: Arc<RwLock<HashMap<TaskId, DingTalkReplyRoute>>>,
}

impl WebState {
    /// 创建新的 Web 状态
    pub fn new(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
    ) -> Self {
        Self::new_with_runtime(
            hub,
            dingtalk_channel,
            llm_client,
            Arc::new(default_agent_runtime()),
        )
    }

    /// 使用指定 Agent 运行时创建 Web 状态
    pub fn new_with_runtime(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        let metrics_collector = Arc::new(MetricsCollector::default());
        let metrics_exporter = Arc::new(MetricsExporter::new(Arc::clone(&metrics_collector)));
        Self::new_with_runtime_and_health(
            hub,
            Arc::new(HealthService::new(env!("CARGO_PKG_VERSION").to_string())),
            metrics_collector,
            metrics_exporter,
            dingtalk_channel,
            llm_client,
            agent_runtime,
        )
    }

    /// 使用显式 health 与 metrics 依赖创建 Web 状态
    pub fn new_with_runtime_and_health(
        hub: Arc<Hub>,
        health_service: Arc<HealthService>,
        metrics_collector: Arc<MetricsCollector>,
        metrics_exporter: Arc<MetricsExporter>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        Self {
            hub,
            health_service,
            metrics_collector,
            metrics_exporter,
            dingtalk_channel,
            llm_client,
            agent_runtime,
            dingtalk_routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

fn default_agent_runtime() -> WebAgentRuntime {
    let agent_manager = AgentManager::new(PathBuf::from("~/.uhorse")).unwrap_or_else(|_| {
        AgentManager::new(std::env::temp_dir().join("uhorse-agent-runtime"))
            .expect("fallback agent manager")
    });
    let memory_store: Arc<dyn MemoryStore> = Arc::new(FileMemory::new(
        std::env::temp_dir().join("uhorse-web-memory"),
    ));

    WebAgentRuntime {
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(HashMap::new()),
        memory_store,
        skills: Arc::new(SkillRegistry::new()),
    }
}

/// 初始化默认 Web Agent 运行时
pub async fn init_default_agent_runtime(
    base_dir: PathBuf,
) -> Result<WebAgentRuntime, Box<dyn std::error::Error + Send + Sync>> {
    let mut agent_manager = AgentManager::new(base_dir.clone())?;
    let workspace_dir = base_dir.join("workspace");
    let scope = AgentScope::new(AgentScopeConfig {
        agent_id: "main".to_string(),
        workspace_dir: workspace_dir.clone(),
        display_name: Some("Main Agent".to_string()),
        is_default: true,
    })?;
    scope.init_workspace().await?;
    agent_manager.register_scope(Arc::new(scope.clone()))?;

    let mut skills = SkillRegistry::new();
    let skills_dir = base_dir.join("skills");
    if skills_dir.exists() {
        let _ = skills.load_from_dir(skills_dir).await;
    }

    let main_agent = Agent::builder()
        .agent_id("main")
        .name("Main Agent")
        .description("Hub default agent")
        .workspace_dir(workspace_dir)
        .system_prompt("You are the default uHorse Hub agent.")
        .set_default(true)
        .build()?
        .with_scope(scope);

    let memory = FileMemory::new(base_dir.join("workspace"));
    memory.init_workspace().await?;

    Ok(WebAgentRuntime {
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(HashMap::from([("main".to_string(), main_agent)])),
        memory_store: Arc::new(memory),
        skills: Arc::new(skills),
    })
}

fn default_agent_id(state: &WebState) -> String {
    if state.agent_runtime.agents.contains_key("main") {
        "main".to_string()
    } else {
        state
            .agent_runtime
            .agents
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "main".to_string())
    }
}

fn agent_scope_for(state: &WebState, agent_id: &str) -> Option<Arc<AgentScope>> {
    state
        .agent_runtime
        .agent_manager
        .get_scope(agent_id)
        .cloned()
        .or_else(|| {
            state
                .agent_runtime
                .agent_manager
                .get_default_scope()
                .cloned()
        })
}

fn build_dingtalk_session_key(
    fallback_user_id: &str,
    sender_user_id: Option<&str>,
    sender_staff_id: Option<&str>,
    sender_corp_id: Option<&str>,
) -> SessionKey {
    let channel_user_id = sender_user_id
        .filter(|value| !value.is_empty())
        .or_else(|| sender_staff_id.filter(|value| !value.is_empty()))
        .unwrap_or(fallback_user_id);

    if let Some(team_id) = sender_corp_id.filter(|value| !value.is_empty()) {
        SessionKey::with_team("dingtalk", channel_user_id, team_id)
    } else {
        SessionKey::new("dingtalk", channel_user_id)
    }
}

async fn resolve_agent_id_for_session(state: &Arc<WebState>, session_key: &SessionKey) -> String {
    let fallback = default_agent_id(state.as_ref());
    let Some(scope) = agent_scope_for(state.as_ref(), &fallback) else {
        return fallback;
    };

    match scope.load_session_state(&session_key.as_str()).await {
        Ok(Some(session_state)) => session_state
            .metadata
            .get("current_agent")
            .filter(|agent_id| state.agent_runtime.agents.contains_key(agent_id.as_str()))
            .cloned()
            .unwrap_or(fallback),
        _ => fallback,
    }
}

async fn collect_agent_planning_context(
    state: &Arc<WebState>,
    agent_id: &str,
    session_key: &SessionKey,
) -> String {
    let core_session_id = CoreSessionId::from_string(session_key.as_str());
    let mut sections = Vec::new();

    if state.agent_runtime.agents.contains_key(agent_id) {
        if let Some(agent) = state.agent_runtime.agents.get(agent_id) {
            if let Some(scope) = agent.scope() {
                let injected_files = scope
                    .get_injected_files(&core_session_id, None)
                    .await
                    .unwrap_or_default();
                if !injected_files.is_empty() {
                    let mut block = String::from("--- Agent Workspace Context ---\n");
                    for (name, content) in injected_files {
                        block.push_str(&format!("\n## {}\n{}\n", name, content));
                    }
                    sections.push(block);
                }
            }
        }
    }

    let memory_context = state
        .agent_runtime
        .memory_store
        .get_context(&core_session_id)
        .await
        .unwrap_or_default();
    if !memory_context.is_empty() {
        sections.push(format!(
            "--- Session Memory Context ---\n{}",
            memory_context
        ));
    }

    sections.join("\n\n")
}

async fn persist_session_state(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
    conversation_id: &str,
    sender_user_id: Option<&str>,
    sender_staff_id: Option<&str>,
    task_id: Option<&TaskId>,
) {
    let Some(scope) = agent_scope_for(state.as_ref(), agent_id) else {
        return;
    };

    let mut session_state = match scope.load_session_state(&session_key.as_str()).await {
        Ok(Some(existing)) => existing,
        _ => SessionState::new(session_key.as_str()),
    };

    session_state.increment_messages();
    session_state
        .metadata
        .insert("current_agent".to_string(), agent_id.to_string());
    session_state
        .metadata
        .insert("conversation_id".to_string(), conversation_id.to_string());
    if let Some(sender_user_id) = sender_user_id {
        session_state
            .metadata
            .insert("sender_user_id".to_string(), sender_user_id.to_string());
    }
    if let Some(sender_staff_id) = sender_staff_id {
        session_state
            .metadata
            .insert("sender_staff_id".to_string(), sender_staff_id.to_string());
    }
    if let Some(task_id) = task_id {
        session_state
            .metadata
            .insert("last_task_id".to_string(), task_id.to_string());
    }

    if let Err(error) = scope
        .save_session_state(&session_key.as_str(), &session_state)
        .await
    {
        warn!(
            "Failed to persist session state for {}: {}",
            session_key, error
        );
    }
}

fn reply_route_from_inbound(inbound: &DingTalkInboundMessage) -> DingTalkReplyRoute {
    DingTalkReplyRoute {
        conversation_id: inbound.conversation_id.clone(),
        conversation_type: inbound.conversation_type.clone(),
        sender_user_id: inbound.sender_user_id.clone(),
        sender_staff_id: inbound.sender_staff_id.clone(),
        session_webhook: inbound.session_webhook.clone(),
        session_webhook_expired_time: inbound.session_webhook_expired_time,
        robot_code: inbound.robot_code.clone(),
    }
}

async fn persist_direct_reply_memory(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
    user_message: &str,
    reply_text: &str,
) {
    let session_id = CoreSessionId::from_string(session_key.as_str());
    if let Err(error) = state
        .agent_runtime
        .memory_store
        .store_message(&session_id, user_message, reply_text)
        .await
    {
        warn!(
            "Failed to persist direct reply memory for {}: {}",
            session_key, error
        );
    }

    let Some(scope) = agent_scope_for(state.as_ref(), agent_id) else {
        return;
    };
    let entry = format!(
        "## {}\n\n**User:** {}\n\n**Assistant:** {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        user_message,
        reply_text
    );
    if let Err(error) = scope.append_to_today_memory(&entry).await {
        warn!(
            "Failed to append today memory for direct reply {}: {}",
            session_key, error
        );
    }

    if let Ok(Some(mut session_state)) = scope.load_session_state(session_id.as_str()).await {
        session_state.touch();
        if let Err(error) = scope
            .save_session_state(session_id.as_str(), &session_state)
            .await
        {
            warn!(
                "Failed to update direct reply session state for {}: {}",
                session_key, error
            );
        }
    }
}

async fn persist_task_result_memory(
    state: &Arc<WebState>,
    completed_task: &CompletedTask,
    reply_text: &str,
) {
    let user_message = completed_task.context.intent.clone().unwrap_or_default();
    if user_message.trim().is_empty() {
        return;
    }

    let session_id =
        CoreSessionId::from_string(completed_task.context.session_id.as_str().to_string());
    if let Err(error) = state
        .agent_runtime
        .memory_store
        .store_message(&session_id, &user_message, reply_text)
        .await
    {
        warn!(
            "Failed to persist session memory for {}: {}",
            completed_task.task_id, error
        );
    }

    let agent_id = completed_task
        .context
        .env
        .get("agent_id")
        .cloned()
        .unwrap_or_else(|| default_agent_id(state.as_ref()));
    let Some(scope) = agent_scope_for(state.as_ref(), &agent_id) else {
        return;
    };

    let entry = format!(
        "## {}\n\n**User:** {}\n\n**Assistant:** {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        user_message,
        reply_text
    );
    if let Err(error) = scope.append_to_today_memory(&entry).await {
        warn!(
            "Failed to append today memory for {}: {}",
            completed_task.task_id, error
        );
    }

    if let Ok(Some(mut session_state)) = scope.load_session_state(session_id.as_str()).await {
        session_state.touch();
        session_state.metadata.insert(
            "last_task_id".to_string(),
            completed_task.task_id.to_string(),
        );
        if let Err(error) = scope
            .save_session_state(session_id.as_str(), &session_state)
            .await
        {
            warn!(
                "Failed to update session state for {}: {}",
                completed_task.task_id, error
            );
        }
    }
}

fn skill_to_summary(skill: &uhorse_agent::Skill) -> SkillRuntimeSummary {
    SkillRuntimeSummary {
        name: skill.manifest.name.clone(),
        description: skill.manifest.description.clone(),
        version: skill.manifest.version.clone(),
        enabled: skill.config.enabled,
        timeout_secs: skill.config.timeout,
        max_retries: skill.config.max_retries,
        executable: skill.config.executable.clone(),
        args: skill.config.args.clone(),
        permissions: skill.manifest.permissions.clone(),
        execution_mode: if skill.config.executable.is_some() {
            "process".to_string()
        } else {
            "dummy".to_string()
        },
    }
}

fn skill_to_detail(skill: &uhorse_agent::Skill) -> SkillRuntimeDetail {
    SkillRuntimeDetail {
        name: skill.manifest.name.clone(),
        description: skill.manifest.description.clone(),
        version: skill.manifest.version.clone(),
        author: skill.manifest.author.clone(),
        enabled: skill.config.enabled,
        timeout_secs: skill.config.timeout,
        max_retries: skill.config.max_retries,
        executable: skill.config.executable.clone(),
        args: skill.config.args.clone(),
        env: skill.config.env.clone(),
        permissions: skill.manifest.permissions.clone(),
        execution_mode: if skill.config.executable.is_some() {
            "process".to_string()
        } else {
            "dummy".to_string()
        },
    }
}

fn session_state_to_detail(session_state: &SessionState) -> SessionRuntimeDetail {
    SessionRuntimeDetail {
        session_id: session_state.session_id.clone(),
        agent_id: session_state.metadata.get("current_agent").cloned(),
        conversation_id: session_state.metadata.get("conversation_id").cloned(),
        sender_user_id: session_state.metadata.get("sender_user_id").cloned(),
        sender_staff_id: session_state.metadata.get("sender_staff_id").cloned(),
        last_task_id: session_state.metadata.get("last_task_id").cloned(),
        message_count: session_state.message_count,
        created_at: session_state.created_at.to_rfc3339(),
        last_active: session_state.last_active.to_rfc3339(),
        metadata: session_state.metadata.clone(),
    }
}

async fn execute_local_skill(
    state: &Arc<WebState>,
    skill_name: &str,
    input: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let skill = state
        .agent_runtime
        .skills
        .get(skill_name)
        .ok_or_else(|| format!("Skill not found: {}", skill_name))?;
    let output = skill.execute(input).await?;
    if output.trim().is_empty() {
        Ok(format!("技能 {} 执行成功，无输出。", skill_name))
    } else {
        Ok(output)
    }
}

async fn collect_runtime_sessions(state: &Arc<WebState>) -> Vec<SessionRuntimeDetail> {
    let mut sessions = HashMap::new();

    for scope in state.agent_runtime.agent_manager.list_agents() {
        let sessions_dir = scope.agents_dir().join("sessions");
        if !sessions_dir.exists() {
            continue;
        }

        let Ok(mut entries) = tokio::fs::read_dir(&sessions_dir).await else {
            continue;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(session_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            let Ok(Some(session_state)) = scope.load_session_state(session_name).await else {
                continue;
            };

            let detail = session_state_to_detail(&session_state);
            let should_replace = sessions
                .get(&detail.session_id)
                .map(|existing: &SessionRuntimeDetail| detail.last_active > existing.last_active)
                .unwrap_or(true);
            if should_replace {
                sessions.insert(detail.session_id.clone(), detail);
            }
        }
    }

    let mut values: Vec<_> = sessions.into_values().collect();
    values.sort_by(|left, right| right.last_active.cmp(&left.last_active));
    values
}

async fn read_session_messages(
    state: &Arc<WebState>,
    session_id: &str,
    agent_id: Option<&str>,
) -> Result<Vec<SessionMessageRecord>, Box<dyn std::error::Error + Send + Sync>> {
    let resolved_agent_id = agent_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| default_agent_id(state.as_ref()));
    let scope = agent_scope_for(state.as_ref(), &resolved_agent_id)
        .ok_or_else(|| format!("Agent scope not found: {}", resolved_agent_id))?;
    let history_path = scope
        .workspace_dir()
        .join("sessions")
        .join(session_id)
        .join("history.md");

    if !history_path.exists() {
        return Ok(vec![]);
    }

    let content = tokio::fs::read_to_string(history_path).await?;
    Ok(parse_session_messages(&content))
}

fn parse_session_messages(content: &str) -> Vec<SessionMessageRecord> {
    let normalized = content.trim();
    if normalized.is_empty() {
        return vec![];
    }

    normalized
        .split("\n## ")
        .filter_map(|chunk| {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                return None;
            }
            let chunk = chunk.strip_prefix("## ").unwrap_or(chunk);
            let (timestamp, rest) = chunk.split_once("\n\n**User:** ")?;
            let (user_message, assistant_part) = rest.split_once("\n\n**Assistant:** ")?;
            Some(SessionMessageRecord {
                timestamp: timestamp.trim().to_string(),
                user_message: user_message.trim().to_string(),
                assistant_message: assistant_part.trim().to_string(),
            })
        })
        .collect()
}

/// Web 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// 监听地址
    pub bind_address: String,
    /// 监听端口
    pub port: u16,
    /// 是否启用 CORS
    pub enable_cors: bool,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 3000,
            enable_cors: true,
        }
    }
}

/// API 响应
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// 是否成功
    pub success: bool,
    /// 数据
    pub data: Option<T>,
    /// 错误信息
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// 创建错误响应
    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

/// 创建 Web 路由
pub fn create_router(state: WebState) -> Router {
    let shared_state = Arc::new(state);
    let metrics_state = Arc::clone(&shared_state);
    let mut router = Router::new()
        // 页面路由
        .route("/", get(index_page))
        .route("/dashboard", get(dashboard_page))
        // WebSocket 路由 (Node 连接)
        .route("/ws", get(ws_handler))
        // DingTalk 回调路由
        .route("/api/v1/channels/dingtalk/webhook", post(dingtalk_webhook))
        .route(
            "/api/v1/channels/dingtalk/webhook",
            get(dingtalk_webhook_verify),
        )
        // API 路由
        .route("/api/stats", get(get_stats))
        .route("/api/nodes", get(list_nodes))
        .route("/api/nodes/:node_id", get(get_node))
        .route(
            "/api/nodes/:node_id/permissions",
            post(update_node_permissions),
        )
        .route("/api/tasks", get(list_tasks).post(submit_task_api))
        .route("/api/tasks/:task_id", get(get_task))
        .route("/api/tasks/:task_id/cancel", post(cancel_task))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/:request_id", get(get_approval))
        .route("/api/approvals/:request_id/approve", post(approve_approval))
        .route("/api/approvals/:request_id/reject", post(reject_approval))
        .route("/api/node-auth/token", post(issue_node_token))
        .route("/api/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/agents", get(list_runtime_agents))
        .route("/api/v1/agents/:agent_id", get(get_runtime_agent))
        .route("/api/v1/skills", get(list_runtime_skills))
        .route("/api/v1/skills/:skill_name", get(get_runtime_skill))
        .route("/api/v1/sessions", get(list_runtime_sessions))
        .route("/api/v1/sessions/:session_id", get(get_runtime_session))
        .route(
            "/api/v1/sessions/:session_id/messages",
            get(get_runtime_session_messages),
        )
        .with_state(shared_state);

    // 添加 CORS、HTTP tracing 与 metrics
    router = router
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            metrics_state,
            track_api_metrics,
        ));

    router
}

/// 首页
async fn index_page() -> Html<&'static str> {
    Html(include_str!("templates/index.html"))
}

/// Dashboard 页面
async fn dashboard_page() -> Html<&'static str> {
    Html(include_str!("templates/dashboard.html"))
}

/// DingTalk webhook 端点
async fn dingtalk_webhook(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    payload: String,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(channel) = state.dingtalk_channel.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "DingTalk channel is not configured"
            })),
        );
    };

    let signature = headers
        .get("x-dingtalk-signature")
        .and_then(|value| value.to_str().ok())
        .or_else(|| headers.get("sign").and_then(|value| value.to_str().ok()));

    match channel.verify_webhook(payload.as_bytes(), signature).await {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Invalid DingTalk signature"
                })),
            );
        }
        Err(error) => {
            error!("Failed to verify DingTalk webhook: {}", error);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": error.to_string()
                })),
            );
        }
    }

    let event: DingTalkEvent = match serde_json::from_str(&payload) {
        Ok(event) => event,
        Err(error) => {
            error!("Failed to parse DingTalk webhook payload: {}", error);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": error.to_string()
                })),
            );
        }
    };

    match channel.handle_event_with_metadata(&event).await {
        Ok(Some(inbound)) => {
            if let Err(error) = submit_dingtalk_task(&state, inbound).await {
                error!("Failed to submit DingTalk task: {}", error);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "status": "error",
                        "message": error.to_string()
                    })),
                );
            }
        }
        Ok(None) => {
            info!("Ignored DingTalk webhook without actionable message");
        }
        Err(error) => {
            error!("Failed to handle DingTalk webhook: {}", error);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": error.to_string()
                })),
            );
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok"
        })),
    )
}

/// DingTalk webhook 验证端点
async fn dingtalk_webhook_verify() -> &'static str {
    "DingTalk webhook endpoint is ready"
}

/// 将 DingTalk 入站消息转换为 Hub 任务并提交执行
pub async fn submit_dingtalk_task(
    state: &Arc<WebState>,
    inbound: DingTalkInboundMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = match &inbound.message.content {
        MessageContent::Text(text) => text.trim(),
        _ => "",
    };

    if text.is_empty() {
        info!(
            "Skip non-text DingTalk message for session {}",
            inbound.session.id
        );
        return Ok(());
    }

    let session_key = build_dingtalk_session_key(
        &inbound.session.channel_user_id,
        inbound.sender_user_id.as_deref(),
        inbound.sender_staff_id.as_deref(),
        inbound.sender_corp_id.as_deref(),
    );
    let agent_id = resolve_agent_id_for_session(state, &session_key).await;
    let route = reply_route_from_inbound(&inbound);
    let decision = decide_dingtalk_action(state, text, &agent_id, &session_key).await?;

    match decision {
        AgentDecision::DirectReply { text: reply_text } => {
            persist_direct_reply_memory(state, &session_key, &agent_id, text, &reply_text).await;
            persist_session_state(
                state,
                &session_key,
                &agent_id,
                &inbound.conversation_id,
                inbound.sender_user_id.as_deref(),
                inbound.sender_staff_id.as_deref(),
                None,
            )
            .await;
            if let Some(channel) = state.dingtalk_channel.as_ref() {
                send_dingtalk_reply(channel, &route, &reply_text).await?;
            } else {
                warn!("Skip DingTalk direct reply because channel is unavailable");
            }
            info!(
                "Replied DingTalk message directly for session {} via agent {}",
                session_key, agent_id
            );
            Ok(())
        }
        AgentDecision::ExecuteSkill { skill_name, input } => {
            let reply_text = execute_local_skill(state, &skill_name, &input).await?;
            persist_direct_reply_memory(state, &session_key, &agent_id, text, &reply_text).await;
            persist_session_state(
                state,
                &session_key,
                &agent_id,
                &inbound.conversation_id,
                inbound.sender_user_id.as_deref(),
                inbound.sender_staff_id.as_deref(),
                None,
            )
            .await;
            if let Some(channel) = state.dingtalk_channel.as_ref() {
                send_dingtalk_reply(channel, &route, &reply_text).await?;
            } else {
                warn!("Skip DingTalk skill reply because channel is unavailable");
            }
            info!(
                "Executed local skill {} for session {} via agent {}",
                skill_name, session_key, agent_id
            );
            Ok(())
        }
        AgentDecision::ExecuteCommand { command } => {
            let Some(node) = state.hub.get_online_nodes().await.into_iter().next() else {
                return Err("No online node available".into());
            };

            let workspace_hint = node.workspace.path.clone();
            validate_planned_command(&command, &workspace_hint)?;

            let task_context = TaskContext::new(
                UserId::from_string(
                    inbound
                        .sender_user_id
                        .clone()
                        .or_else(|| inbound.sender_staff_id.clone())
                        .unwrap_or_else(|| inbound.session.channel_user_id.clone()),
                ),
                uhorse_protocol::SessionId::from_string(session_key.as_str()),
                "dingtalk",
            )
            .with_intent(text.to_string())
            .with_env("agent_id", agent_id.clone())
            .with_env("conversation_id", inbound.conversation_id.clone());

            let task_id = state
                .hub
                .submit_task(
                    command,
                    task_context,
                    uhorse_protocol::Priority::Normal,
                    None,
                    vec![],
                    Some(workspace_hint),
                )
                .await?;

            persist_session_state(
                state,
                &session_key,
                &agent_id,
                &inbound.conversation_id,
                inbound.sender_user_id.as_deref(),
                inbound.sender_staff_id.as_deref(),
                Some(&task_id),
            )
            .await;

            {
                let mut routes = state.dingtalk_routes.write().await;
                routes.insert(task_id.clone(), route);
            }

            info!(
                "Submitted DingTalk task {} for session {} via agent {}",
                task_id, session_key, agent_id
            );

            Ok(())
        }
    }
}

async fn decide_dingtalk_action(
    state: &Arc<WebState>,
    text: &str,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<AgentDecision, Box<dyn std::error::Error + Send + Sync>> {
    let Some(llm_client) = state.llm_client.as_ref() else {
        return Err("LLM client is not configured".into());
    };
    let agent = state
        .agent_runtime
        .agents
        .get(agent_id)
        .ok_or_else(|| format!("Agent not found: {}", agent_id))?;
    let context = collect_agent_planning_context(state, agent_id, session_key).await;
    let response = llm_client
        .chat_completion(build_agent_decision_messages(
            agent.system_prompt(),
            text,
            agent_id,
            session_key,
            &context,
            &state.agent_runtime.skills.list_names(),
        ))
        .await?;

    parse_agent_decision(state, &response, text, agent_id, session_key).await
}

async fn parse_agent_decision(
    state: &Arc<WebState>,
    response: &str,
    text: &str,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<AgentDecision, Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(decision) = serde_json::from_str::<AgentDecision>(response) {
        return Ok(decision);
    }

    let Some(node) = state.hub.get_online_nodes().await.into_iter().next() else {
        return Ok(AgentDecision::DirectReply {
            text: response.trim().to_string(),
        });
    };
    let workspace_root = node.workspace.path.clone();

    if let Ok(command) = parse_planned_command(response, &workspace_root) {
        return Ok(AgentDecision::ExecuteCommand { command });
    }

    let trimmed = response.trim();
    if !trimmed.is_empty() {
        return Ok(AgentDecision::DirectReply {
            text: trimmed.to_string(),
        });
    }

    plan_dingtalk_command(state, text, &workspace_root, agent_id, session_key).await
}

async fn plan_dingtalk_command(
    state: &Arc<WebState>,
    text: &str,
    workspace_root: &str,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<AgentDecision, Box<dyn std::error::Error + Send + Sync>> {
    let Some(llm_client) = state.llm_client.as_ref() else {
        return Err("LLM client is not configured".into());
    };

    let injected_context = collect_agent_planning_context(state, agent_id, session_key).await;
    let response = llm_client
        .chat_completion(build_dingtalk_plan_messages(
            text,
            workspace_root,
            agent_id,
            session_key,
            &injected_context,
        ))
        .await?;

    let command = parse_planned_command(&response, workspace_root)?;
    Ok(AgentDecision::ExecuteCommand { command })
}

fn build_agent_decision_messages(
    agent_system_prompt: &str,
    text: &str,
    agent_id: &str,
    session_key: &SessionKey,
    injected_context: &str,
    skill_names: &[String],
) -> Vec<ChatMessage> {
    let mut messages = vec![
        ChatMessage::system(agent_system_prompt.to_string()),
        ChatMessage::system(
            format!(
                "你是 uHorse Hub 的 Agent 决策器。你必须根据用户输入与上下文，只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。允许三种结构：1）直接回复：{{\"type\":\"direct_reply\",\"text\":\"...\"}}；2）需要继续规划命令：{{\"type\":\"execute_command\",\"command\": <uhorse_protocol::Command JSON>}}；3）执行 Hub 本地技能：{{\"type\":\"execute_skill\",\"skill_name\":\"...\",\"input\":\"...\"}}。优先 direct_reply；只有确实需要 Node 执行文件或 shell 操作时才返回 execute_command。只有当请求明确适合本地技能时才返回 execute_skill。可用技能列表：{}。禁止生成 code/database/api/browser 命令。若返回 execute_command，路径必须限制在 workspace 内，不允许绝对路径越界，不允许使用 ..。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。",
                if skill_names.is_empty() {
                    "（无）".to_string()
                } else {
                    skill_names.join(", ")
                }
            )
        ),
    ];

    if !injected_context.trim().is_empty() {
        messages.push(ChatMessage::system(format!(
            "当前 Agent：{}\n当前 SessionKey：{}\n{}",
            agent_id,
            session_key.as_str(),
            injected_context
        )));
    }

    messages.push(ChatMessage::user(format!(
        "agent_id: {}\nsession_key: {}\nuser_request: {}\n请输出单个 JSON 对象。",
        agent_id,
        session_key.as_str(),
        text
    )));
    messages
}

fn build_dingtalk_plan_messages(
    text: &str,
    workspace_root: &str,
    agent_id: &str,
    session_key: &SessionKey,
    injected_context: &str,
) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage::system(
        "你是 uHorse Hub 的任务规划器。你必须把用户的自然语言请求转换为单个 JSON 对象，且只能输出 JSON，不要输出 Markdown、解释或代码块。JSON 结构必须是 {\"command\": <uhorse_protocol::Command JSON> }。优先生成 file 命令；只有文件命令无法完成时才生成 shell 命令。禁止生成 code/database/api/browser/skill 命令。路径必须限制在 workspace 内，不允许绝对路径越界，不允许使用 ..。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。如果无法安全规划，返回一个会在本地校验失败的命令并附带原因到路径字段之外是不允许的，因此应返回最接近且可解析的安全命令。shell 命令只允许只读、安全的本地仓库检查或目录查看。".to_string(),
    )];

    if !injected_context.trim().is_empty() {
        messages.push(ChatMessage::system(format!(
            "当前 Agent：{}\n当前 SessionKey：{}\n{}",
            agent_id,
            session_key.as_str(),
            injected_context
        )));
    }

    messages.push(ChatMessage::user(format!(
        "workspace_root: {}\nagent_id: {}\nsession_key: {}\nuser_request: {}\n请输出单个 JSON 对象。",
        workspace_root,
        agent_id,
        session_key.as_str(),
        text
    )));
    messages
}

fn parse_planned_command(
    response: &str,
    workspace_root: &str,
) -> Result<Command, Box<dyn std::error::Error + Send + Sync>> {
    let planned: PlannedDingTalkCommand =
        serde_json::from_str(response).map_err(|e| format!("LLM 返回了无效 JSON：{}", e))?;

    validate_planned_command(&planned.command, workspace_root)?;
    Ok(planned.command)
}

fn validate_planned_command(
    command: &Command,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        Command::File(file_command) => validate_file_command(file_command, workspace_root),
        Command::Shell(shell_command) => validate_shell_command(shell_command, workspace_root),
        _ => Err("仅允许规划 FileCommand 或 ShellCommand。".into()),
    }
}

fn validate_file_command(
    command: &FileCommand,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        FileCommand::Read { path, .. }
        | FileCommand::Write { path, .. }
        | FileCommand::Append { path, .. }
        | FileCommand::Delete { path, .. }
        | FileCommand::List { path, .. }
        | FileCommand::Search { path, .. }
        | FileCommand::Info { path }
        | FileCommand::CreateDir { path, .. }
        | FileCommand::Exists { path } => {
            ensure_workspace_path(path, workspace_root)?;
        }
        FileCommand::Copy { from, to, .. } | FileCommand::Move { from, to, .. } => {
            ensure_workspace_path(from, workspace_root)?;
            ensure_workspace_path(to, workspace_root)?;
        }
    }

    Ok(())
}

fn validate_shell_command(
    command: &uhorse_protocol::ShellCommand,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(cwd) = command.cwd.as_deref() {
        ensure_workspace_path(cwd, workspace_root)?;
    }

    let command_text = std::iter::once(command.command.as_str())
        .chain(command.args.iter().map(|arg| arg.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    for pattern in [
        "git reset --hard",
        "git clean -fd",
        "git clean -f -d",
        "git checkout --",
        "git restore --source",
        "git push --force",
        "git push -f",
    ] {
        if command_text.contains(pattern) {
            return Err(format!("禁止危险 git 命令：{}", pattern).into());
        }
    }

    Ok(())
}

fn ensure_workspace_path(
    value: &str,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let root = PathBuf::from(workspace_root);
    let path = PathBuf::from(value);

    if path.is_absolute() {
        if !path.starts_with(&root) {
            return Err("路径必须位于 workspace 内。".into());
        }
        return Ok(());
    }

    normalize_relative_path(value)?;
    Ok(())
}

fn normalize_relative_path(
    value: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = FsPath::new(value);
    if path.is_absolute() {
        return Err("路径必须是 workspace 内的相对路径。".into());
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("不允许使用 .. 或绝对路径。".into())
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(normalized.to_string_lossy().to_string())
    }
}

/// 将任务结果回传到对应的 DingTalk 会话
pub async fn reply_task_result(
    state: Arc<WebState>,
    task_result: TaskResult,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let completed_task = state.hub.get_completed_task(&task_result.task_id).await;
    let reply_text = build_task_result_reply_text(&state, &task_result).await;

    if let Some(completed_task) = completed_task.as_ref() {
        persist_task_result_memory(&state, completed_task, &reply_text).await;
    }

    let Some(channel) = state.dingtalk_channel.as_ref() else {
        warn!("Skip DingTalk reply because channel is unavailable");
        return Ok(());
    };

    let route = {
        let mut routes = state.dingtalk_routes.write().await;
        routes.remove(&task_result.task_id)
    };

    let Some(route) = route else {
        return Ok(());
    };

    send_dingtalk_reply(channel, &route, &reply_text).await?;

    info!("Replied DingTalk task result for {}", task_result.task_id);
    Ok(())
}

/// 将任务提交阶段的错误回传到 DingTalk 会话
pub async fn reply_dingtalk_error(
    state: &Arc<WebState>,
    inbound: &DingTalkInboundMessage,
    error_message: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(channel) = state.dingtalk_channel.as_ref() else {
        warn!("Skip DingTalk error reply because channel is unavailable");
        return Ok(());
    };

    let route = DingTalkReplyRoute {
        conversation_id: inbound.conversation_id.clone(),
        conversation_type: inbound.conversation_type.clone(),
        sender_user_id: inbound.sender_user_id.clone(),
        sender_staff_id: inbound.sender_staff_id.clone(),
        session_webhook: inbound.session_webhook.clone(),
        session_webhook_expired_time: inbound.session_webhook_expired_time,
        robot_code: inbound.robot_code.clone(),
    };
    let reply_text = format!("执行失败：{}", error_message);

    send_dingtalk_reply(channel, &route, &reply_text).await?;

    info!(
        "Replied DingTalk immediate error for conversation {}",
        route.conversation_id
    );
    Ok(())
}

async fn send_dingtalk_reply(
    channel: &DingTalkChannel,
    route: &DingTalkReplyRoute,
    reply_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let webhook_available = route
        .session_webhook
        .as_deref()
        .zip(route.session_webhook_expired_time)
        .map(|(_, expires_at)| now_ms < expires_at)
        .unwrap_or(false);

    if webhook_available {
        let at_user_ids = route
            .sender_staff_id
            .as_ref()
            .map(|value| vec![value.clone()])
            .unwrap_or_default();
        channel
            .reply_via_session_webhook(
                route.session_webhook.as_deref().unwrap_or_default(),
                reply_text,
                &at_user_ids,
            )
            .await?;
    } else {
        let is_group = matches!(route.conversation_type.as_deref(), Some("2"));
        if is_group {
            channel
                .send_group_message(
                    &route.conversation_id,
                    &MessageContent::Text(reply_text.to_string()),
                )
                .await?;
        } else if let Some(user_id) = route.sender_user_id.as_deref() {
            channel
                .send_message(user_id, &MessageContent::Text(reply_text.to_string()))
                .await?;
        } else {
            warn!(
                "Skip DingTalk personal reply for conversation {} because sender_user_id is missing",
                route.conversation_id
            );
        }
    }

    Ok(())
}

async fn build_task_result_reply_text(state: &Arc<WebState>, task_result: &TaskResult) -> String {
    let Some(completed_task) = state.hub.get_completed_task(&task_result.task_id).await else {
        return format_task_result_message(&task_result.result);
    };

    summarize_task_result_or_fallback(state, &completed_task).await
}

async fn summarize_task_result_or_fallback(
    state: &Arc<WebState>,
    completed_task: &CompletedTask,
) -> String {
    match summarize_task_result(state, completed_task).await {
        Ok(summary) => summary,
        Err(error) => {
            warn!(
                "Failed to summarize task result with LLM for {}: {}",
                completed_task.task_id, error
            );
            format_task_result_message(&completed_task.result)
        }
    }
}

async fn summarize_task_result(
    state: &Arc<WebState>,
    completed_task: &CompletedTask,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let Some(llm_client) = state.llm_client.as_ref() else {
        return Err("LLM client is not configured".into());
    };

    let response = llm_client
        .chat_completion(build_result_summary_messages(completed_task))
        .await?;

    let summary = response.trim();
    if summary.is_empty() {
        return Err("LLM returned empty summary".into());
    }

    Ok(summary.to_string())
}

fn build_result_summary_messages(completed_task: &CompletedTask) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(
            "你是 DingTalk 任务执行结果总结助手。请基于用户原始意图、实际执行命令和执行结果，生成简短、自然的中文回复。要求：1）直接回答结果；2）成功时说明关键发现；3）失败时说明失败原因；4）不要输出 Markdown 代码块；5）不要编造未执行的信息。".to_string(),
        ),
        ChatMessage::user(format!(
            "user_intent: {}\ncommand: {}\nresult: {}",
            completed_task.context.intent.clone().unwrap_or_default(),
            serde_json::to_string(&completed_task.command).unwrap_or_else(|_| "{}".to_string()),
            serde_json::to_string(&completed_task.result).unwrap_or_else(|_| "{}".to_string())
        )),
    ]
}

fn format_task_result_message(result: &uhorse_protocol::CommandResult) -> String {
    if !result.success {
        if let Some(skill_name) = result
            .metadata
            .get("skill_name")
            .and_then(|value| value.as_str())
        {
            return result
                .error
                .as_ref()
                .map(|error| format!("技能 {} 执行失败：{}", skill_name, error.message))
                .unwrap_or_else(|| format!("技能 {} 执行失败。", skill_name));
        }

        return result
            .error
            .as_ref()
            .map(|error| format!("执行失败：{}", error.message))
            .unwrap_or_else(|| "执行失败。".to_string());
    }

    match &result.output {
        CommandOutput::Text { content } => {
            if content.trim().is_empty() {
                "执行成功，无输出。".to_string()
            } else {
                content.clone()
            }
        }
        CommandOutput::Json { content } => {
            serde_json::to_string_pretty(content).unwrap_or_else(|_| content.to_string())
        }
        CommandOutput::None => "执行成功，无输出。".to_string(),
        other => format!("执行成功，输出类型：{:?}", other),
    }
}

/// 获取统计信息
async fn get_stats(State(state): State<Arc<WebState>>) -> Json<ApiResponse<HubStats>> {
    let stats = state.hub.get_stats().await;
    Json(ApiResponse::success(stats))
}

async fn list_runtime_agents(
    State(state): State<Arc<WebState>>,
) -> Json<ApiResponse<Vec<AgentRuntimeSummary>>> {
    let sessions = collect_runtime_sessions(&state).await;
    let session_counts: HashMap<String, usize> =
        sessions.into_iter().fold(HashMap::new(), |mut acc, item| {
            if let Some(agent_id) = item.agent_id {
                *acc.entry(agent_id).or_insert(0) += 1;
            }
            acc
        });

    let mut agents: Vec<_> = state
        .agent_runtime
        .agents
        .values()
        .map(|agent| AgentRuntimeSummary {
            agent_id: agent.agent_id().to_string(),
            name: agent.name().to_string(),
            description: agent.description().to_string(),
            workspace_dir: agent.workspace_dir().display().to_string(),
            is_default: state
                .agent_runtime
                .agent_manager
                .get_default_scope()
                .map(|scope| scope.config().agent_id == agent.agent_id())
                .unwrap_or(false),
            skill_names: state.agent_runtime.skills.list_names(),
            active_session_count: session_counts.get(agent.agent_id()).copied().unwrap_or(0),
        })
        .collect();
    agents.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));

    Json(ApiResponse::success(agents))
}

async fn get_runtime_agent(
    State(state): State<Arc<WebState>>,
    Path(agent_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<AgentRuntimeDetail>>) {
    let sessions = collect_runtime_sessions(&state).await;
    let active_session_count = sessions
        .into_iter()
        .filter(|session| session.agent_id.as_deref() == Some(agent_id.as_str()))
        .count();

    match state.agent_runtime.agents.get(&agent_id) {
        Some(agent) => (
            StatusCode::OK,
            Json(ApiResponse::success(AgentRuntimeDetail {
                agent_id: agent.agent_id().to_string(),
                name: agent.name().to_string(),
                description: agent.description().to_string(),
                workspace_dir: agent.workspace_dir().display().to_string(),
                system_prompt: agent.system_prompt().to_string(),
                is_default: state
                    .agent_runtime
                    .agent_manager
                    .get_default_scope()
                    .map(|scope| scope.config().agent_id == agent.agent_id())
                    .unwrap_or(false),
                skill_names: state.agent_runtime.skills.list_names(),
                active_session_count,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Agent not found")),
        ),
    }
}

async fn list_runtime_skills(
    State(state): State<Arc<WebState>>,
) -> Json<ApiResponse<Vec<SkillRuntimeSummary>>> {
    let mut skills: Vec<_> = state
        .agent_runtime
        .skills
        .list_names()
        .into_iter()
        .filter_map(|name| state.agent_runtime.skills.get(&name))
        .map(|skill| skill_to_summary(&skill))
        .collect();
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Json(ApiResponse::success(skills))
}

async fn get_runtime_skill(
    State(state): State<Arc<WebState>>,
    Path(skill_name): Path<String>,
) -> (StatusCode, Json<ApiResponse<SkillRuntimeDetail>>) {
    match state.agent_runtime.skills.get(&skill_name) {
        Some(skill) => (
            StatusCode::OK,
            Json(ApiResponse::success(skill_to_detail(&skill))),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Skill not found")),
        ),
    }
}

async fn list_runtime_sessions(
    State(state): State<Arc<WebState>>,
) -> Json<ApiResponse<Vec<SessionRuntimeSummary>>> {
    let sessions = collect_runtime_sessions(&state)
        .await
        .into_iter()
        .map(|session| SessionRuntimeSummary {
            session_id: session.session_id,
            agent_id: session.agent_id,
            conversation_id: session.conversation_id,
            sender_user_id: session.sender_user_id,
            sender_staff_id: session.sender_staff_id,
            last_task_id: session.last_task_id,
            message_count: session.message_count,
            created_at: session.created_at,
            last_active: session.last_active,
        })
        .collect();
    Json(ApiResponse::success(sessions))
}

async fn get_runtime_session(
    State(state): State<Arc<WebState>>,
    Path(session_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<SessionRuntimeDetail>>) {
    let sessions = collect_runtime_sessions(&state).await;
    match sessions
        .into_iter()
        .find(|session| session.session_id == session_id)
    {
        Some(session) => (StatusCode::OK, Json(ApiResponse::success(session))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Session not found")),
        ),
    }
}

async fn get_runtime_session_messages(
    State(state): State<Arc<WebState>>,
    Path(session_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Vec<SessionMessageRecord>>>) {
    let sessions = collect_runtime_sessions(&state).await;
    let Some(session) = sessions
        .into_iter()
        .find(|item| item.session_id == session_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Session not found")),
        );
    };

    match read_session_messages(&state, &session_id, session.agent_id.as_deref()).await {
        Ok(messages) => (StatusCode::OK, Json(ApiResponse::success(messages))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

/// 列出所有节点
async fn list_nodes(State(state): State<Arc<WebState>>) -> Json<ApiResponse<Vec<crate::NodeInfo>>> {
    let nodes = state.hub.get_all_nodes().await;
    Json(ApiResponse::success(nodes))
}

/// 获取单个节点
async fn get_node(
    State(state): State<Arc<WebState>>,
    Path(node_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Option<crate::NodeInfo>>>) {
    let nodes = state.hub.get_all_nodes().await;
    match nodes.into_iter().find(|n| n.node_id.as_str() == node_id) {
        Some(node) => (StatusCode::OK, Json(ApiResponse::success(Some(node)))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Node not found")),
        ),
    }
}

/// 任务信息
#[derive(Debug, Serialize)]
pub struct TaskInfo {
    /// 任务 ID
    pub task_id: String,
    /// 状态
    pub status: String,
    /// 命令类型
    pub command_type: String,
    /// 优先级
    pub priority: String,
    /// 开始时间
    pub started_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApprovalDecisionPayload {
    responder: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubmitTaskPayload {
    command: Command,
    user_id: String,
    session_id: String,
    channel: String,
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    priority: Priority,
    #[serde(default)]
    workspace_hint: Option<String>,
    #[serde(default)]
    required_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SubmitTaskResponse {
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodePermissionUpdatePayload {
    rules: Vec<ProtocolPermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IssueNodeTokenPayload {
    node_id: String,
    credentials: String,
}

#[derive(Debug, Clone, Serialize)]
struct IssueNodeTokenResponse {
    node_id: String,
    access_token: String,
    refresh_token: String,
    expires_at: String,
}

/// 列出任务
async fn list_tasks(State(_state): State<Arc<WebState>>) -> Json<ApiResponse<Vec<TaskInfo>>> {
    // 简化实现：返回空列表
    Json(ApiResponse::success(vec![]))
}

async fn submit_task_api(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<SubmitTaskPayload>,
) -> (StatusCode, Json<ApiResponse<SubmitTaskResponse>>) {
    let mut context = TaskContext::new(
        UserId::from_string(payload.user_id),
        SessionId::from_string(payload.session_id),
        payload.channel,
    );
    if let Some(intent) = payload.intent {
        context = context.with_intent(intent);
    }
    for (key, value) in payload.env {
        context = context.with_env(key, value);
    }

    match state
        .hub
        .submit_task(
            payload.command,
            context,
            payload.priority,
            None,
            payload.required_tags,
            payload.workspace_hint,
        )
        .await
    {
        Ok(task_id) => (
            StatusCode::CREATED,
            Json(ApiResponse::success(SubmitTaskResponse {
                task_id: task_id.to_string(),
            })),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn issue_node_token(
    State(state): State<Arc<WebState>>,
    Json(payload): Json<IssueNodeTokenPayload>,
) -> (StatusCode, Json<ApiResponse<IssueNodeTokenResponse>>) {
    let Some(security_manager) = state.hub.security_manager() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Security manager not configured")),
        );
    };

    match security_manager
        .node_authenticator()
        .authenticate_node(
            &uhorse_protocol::NodeId::from_string(payload.node_id.clone()),
            &payload.credentials,
        )
        .await
    {
        Ok(auth_info) => (
            StatusCode::OK,
            Json(ApiResponse::success(IssueNodeTokenResponse {
                node_id: auth_info.node_id.to_string(),
                access_token: auth_info.access_token,
                refresh_token: auth_info.refresh_token,
                expires_at: auth_info.expires_at.to_rfc3339(),
            })),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn update_node_permissions(
    State(state): State<Arc<WebState>>,
    Path(node_id): Path<String>,
    Json(payload): Json<NodePermissionUpdatePayload>,
) -> (StatusCode, Json<ApiResponse<&'static str>>) {
    let node_id = uhorse_protocol::NodeId::from_string(node_id);
    let senders = state.hub.message_router().node_senders();
    let sender = {
        let senders = senders.read().await;
        match senders.get(&node_id).cloned() {
            Some(sender) => sender,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::error("Node not found")),
                )
            }
        }
    };

    match state
        .hub
        .message_router()
        .send_to_node(
            &node_id,
            HubToNode::PermissionUpdate {
                message_id: MessageId::new(),
                rules: payload.rules,
            },
            &sender,
        )
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(ApiResponse::success("Permission update sent")),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

/// 获取单个任务
async fn get_task(
    State(state): State<Arc<WebState>>,
    Path(task_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Option<TaskInfo>>>) {
    match state
        .hub
        .get_task_status(&uhorse_protocol::TaskId::from_string(&task_id))
        .await
    {
        Some(status) => {
            let info = TaskInfo {
                task_id: status.task_id.to_string(),
                status: format!("{:?}", status.status),
                command_type: status
                    .command_type
                    .map(|command_type| format!("{:?}", command_type).to_lowercase())
                    .unwrap_or_else(|| "unknown".to_string()),
                priority: status
                    .priority
                    .map(|priority| priority.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                started_at: status.started_at.map(|t| t.to_rfc3339()),
            };
            (StatusCode::OK, Json(ApiResponse::success(Some(info))))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Task not found")),
        ),
    }
}

/// 取消任务
async fn cancel_task(
    State(state): State<Arc<WebState>>,
    Path(task_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<&'static str>>) {
    match state
        .hub
        .cancel_task(
            &uhorse_protocol::TaskId::from_string(&task_id),
            "User cancelled",
        )
        .await
    {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::success("Task cancelled"))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&e.to_string())),
        ),
    }
}

/// 列出待审批
async fn list_approvals(
    State(state): State<Arc<WebState>>,
) -> (StatusCode, Json<ApiResponse<Vec<ApprovalRequest>>>) {
    let Some(security_manager) = state.hub.security_manager() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Security manager not configured")),
        );
    };

    match security_manager
        .operation_approver()
        .list_pending_requests()
        .await
    {
        Ok(requests) => (StatusCode::OK, Json(ApiResponse::success(requests))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

/// 获取单个审批
async fn get_approval(
    State(state): State<Arc<WebState>>,
    Path(request_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<ApprovalRequest>>) {
    let Some(security_manager) = state.hub.security_manager() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Security manager not configured")),
        );
    };

    match security_manager
        .operation_approver()
        .get_request(&request_id)
        .await
    {
        Ok(Some(request)) => (StatusCode::OK, Json(ApiResponse::success(request))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Approval not found")),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

/// 批准审批
async fn approve_approval(
    State(state): State<Arc<WebState>>,
    Path(request_id): Path<String>,
    Json(payload): Json<ApprovalDecisionPayload>,
) -> (StatusCode, Json<ApiResponse<ApprovalRequest>>) {
    decide_approval(state, request_id, payload, true).await
}

/// 拒绝审批
async fn reject_approval(
    State(state): State<Arc<WebState>>,
    Path(request_id): Path<String>,
    Json(payload): Json<ApprovalDecisionPayload>,
) -> (StatusCode, Json<ApiResponse<ApprovalRequest>>) {
    decide_approval(state, request_id, payload, false).await
}

async fn decide_approval(
    state: Arc<WebState>,
    request_id: String,
    payload: ApprovalDecisionPayload,
    approved: bool,
) -> (StatusCode, Json<ApiResponse<ApprovalRequest>>) {
    let Some(security_manager) = state.hub.security_manager() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Security manager not configured")),
        );
    };

    let existing_request = match security_manager
        .operation_approver()
        .get_request(&request_id)
        .await
    {
        Ok(Some(request)) => request,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::error("Approval not found")),
            );
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&error.to_string())),
            );
        }
    };

    let decision_result = if approved {
        security_manager
            .operation_approver()
            .approve(&request_id, &payload.responder, payload.reason.as_deref())
            .await
    } else {
        security_manager
            .operation_approver()
            .reject(&request_id, &payload.responder, payload.reason.as_deref())
            .await
    };

    if let Err(error) = decision_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        );
    }

    let updated_request = match security_manager
        .operation_approver()
        .get_request(&request_id)
        .await
    {
        Ok(Some(request)) => request,
        Ok(None) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Approval disappeared after decision")),
            );
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(&error.to_string())),
            );
        }
    };

    if let Err(error) =
        notify_node_approval_decision(&state, &existing_request, &payload, approved).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        );
    }

    (StatusCode::OK, Json(ApiResponse::success(updated_request)))
}

async fn notify_node_approval_decision(
    state: &Arc<WebState>,
    request: &ApprovalRequest,
    payload: &ApprovalDecisionPayload,
    approved: bool,
) -> Result<(), crate::HubError> {
    let Some(node_id) = request
        .metadata
        .get("node_id")
        .and_then(|value| value.as_str())
        .map(uhorse_protocol::NodeId::from_string)
    else {
        return Err(crate::HubError::Internal(
            "Approval request missing node_id metadata".to_string(),
        ));
    };

    let request_id = request
        .metadata
        .get("request_id")
        .and_then(|value| value.as_str())
        .unwrap_or(request.id.as_str())
        .to_string();

    let senders = state.hub.message_router().node_senders();
    let sender = {
        let senders = senders.read().await;
        senders
            .get(&node_id)
            .cloned()
            .ok_or_else(|| crate::HubError::NodeNotFound(node_id.clone()))?
    };

    state
        .hub
        .message_router()
        .send_to_node(
            &node_id,
            HubToNode::ApprovalResponse {
                message_id: MessageId::new(),
                request_id,
                approved,
                responder: payload.responder.clone(),
                reason: payload.reason.clone(),
                responded_at: Utc::now(),
            },
            &sender,
        )
        .await
}

/// 健康检查
async fn health_check(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let health = state.health_service.check().await;
    let status = match health.status {
        HealthStatus::Healthy => "healthy",
        HealthStatus::Degraded => "degraded",
        HealthStatus::Unhealthy => "unhealthy",
    };

    Json(serde_json::json!({
        "status": status,
        "version": health.version,
    }))
}

async fn metrics_handler(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let mut body = state.metrics_exporter.export_metrics().await;
    let stats = state.hub.get_stats().await;
    body.push_str(&format_hub_metrics(&stats));
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

async fn track_api_metrics(
    State(state): State<Arc<WebState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| request.uri().path())
        .to_string();
    let timer =
        uhorse_observability::ApiTimer::new(path, method, Arc::clone(&state.metrics_collector));
    let response = next.run(request).await;
    let status = response.status().as_u16();
    timer.complete_with_status(status).await;
    response
}

fn format_hub_metrics(stats: &HubStats) -> String {
    format!(
        "# HELP uhorse_hub_uptime_seconds Hub uptime in seconds.\n\
# TYPE uhorse_hub_uptime_seconds gauge\n\
uhorse_hub_uptime_seconds {}\n\
# HELP uhorse_hub_nodes_total Total number of registered nodes.\n\
# TYPE uhorse_hub_nodes_total gauge\n\
uhorse_hub_nodes_total {}\n\
# HELP uhorse_hub_nodes_online Number of online nodes.\n\
# TYPE uhorse_hub_nodes_online gauge\n\
uhorse_hub_nodes_online {}\n\
# HELP uhorse_hub_nodes_offline Number of offline nodes.\n\
# TYPE uhorse_hub_nodes_offline gauge\n\
uhorse_hub_nodes_offline {}\n\
# HELP uhorse_hub_nodes_busy Number of busy nodes.\n\
# TYPE uhorse_hub_nodes_busy gauge\n\
uhorse_hub_nodes_busy {}\n\
# HELP uhorse_hub_tasks_pending Number of pending tasks.\n\
# TYPE uhorse_hub_tasks_pending gauge\n\
uhorse_hub_tasks_pending {}\n\
# HELP uhorse_hub_tasks_running Number of running tasks.\n\
# TYPE uhorse_hub_tasks_running gauge\n\
uhorse_hub_tasks_running {}\n\
# HELP uhorse_hub_tasks_completed Number of completed tasks.\n\
# TYPE uhorse_hub_tasks_completed gauge\n\
uhorse_hub_tasks_completed {}\n\
# HELP uhorse_hub_tasks_failed Number of failed tasks.\n\
# TYPE uhorse_hub_tasks_failed gauge\n\
uhorse_hub_tasks_failed {}\n",
        stats.uptime_secs,
        stats.nodes.total_nodes,
        stats.nodes.online_nodes,
        stats.nodes.offline_nodes,
        stats.nodes.busy_nodes,
        stats.scheduler.pending_tasks,
        stats.scheduler.running_tasks,
        stats.scheduler.completed_tasks,
        stats.scheduler.failed_tasks,
    )
}

/// 启动 Web 服务器
pub async fn start_server(
    config: WebConfig,
    hub: Arc<Hub>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = WebState::new(hub, None, None);
    let app = create_router(state);

    let addr = format!("{}:{}", config.bind_address, config.port);
    info!("Web server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::json;
    use std::time::Duration;
    use tempfile::{tempdir, TempDir};
    use tower::util::ServiceExt;
    use uhorse_protocol::{
        Action, Command, CommandOutput, CommandResult, ExecutionError, FileCommand,
        NodeCapabilities, Priority, ResourcePattern as ProtocolResourcePattern, ShellCommand,
        TaskStatus, WorkspaceInfo,
    };

    use crate::HubConfig;

    async fn create_test_runtime() -> Arc<WebAgentRuntime> {
        let base_dir = tempdir().unwrap().keep();
        Arc::new(
            init_default_agent_runtime(base_dir.join("agent-runtime"))
                .await
                .unwrap(),
        )
    }

    async fn create_test_runtime_with_skill(
        skill_name: &str,
        skill_toml: &str,
        llm_response: &str,
    ) -> (Arc<WebAgentRuntime>, Arc<WebState>) {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let skill_dir = runtime_root.join("skills").join(skill_name);
        tokio::fs::create_dir_all(&skill_dir).await.unwrap();
        tokio::fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: {}\nversion: 1.0.0\ndescription: {} skill\nauthor: test\nparameters: []\npermissions: []\n---\n",
                skill_name, skill_name
            ),
        )
        .await
        .unwrap();
        tokio::fs::write(skill_dir.join("skill.toml"), skill_toml)
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: llm_response.to_string(),
            })),
            runtime.clone(),
        ));

        (runtime, state)
    }

    struct StubLlmClient {
        response: String,
    }

    #[async_trait::async_trait]
    impl LLMClient for StubLlmClient {
        async fn chat_completion(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    struct FailingLlmClient;

    #[async_trait::async_trait]
    impl LLMClient for FailingLlmClient {
        async fn chat_completion(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Err(anyhow::anyhow!("llm failed"))
        }
    }

    async fn create_security_test_state() -> (
        Arc<WebState>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
    ) {
        let security_manager = Arc::new(
            crate::security_integration::SecurityManager::new(
                "jwt-secret",
                Arc::new(uhorse_security::ApprovalManager::new()),
            )
            .unwrap(),
        );
        let (hub, _rx) = Hub::new_with_security(HubConfig::default(), Some(security_manager));
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new(hub.clone(), None, None));
        let node_id = uhorse_protocol::NodeId::from_string("node-approval-web");
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        (state, node_id, rx)
    }

    async fn create_registered_node_test_state() -> (
        Arc<WebState>,
        Arc<Hub>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
        TempDir,
    ) {
        let workspace = tempdir().unwrap();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new(hub.clone(), None, None));
        let node_id = uhorse_protocol::NodeId::from_string("node-web-runtime");
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id.clone(),
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                name: "workspace".to_string(),
                path: workspace.path().to_string_lossy().to_string(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        (state, hub, node_id, rx, workspace)
    }

    async fn create_router_test_state_with_security() -> (
        Router,
        Arc<WebState>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
    ) {
        let (state, node_id, rx) = create_security_test_state().await;
        let app = create_router((*state.as_ref()).clone());
        (app, state, node_id, rx)
    }

    async fn create_router_test_state_with_registered_node() -> (
        Router,
        Arc<WebState>,
        Arc<Hub>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
        TempDir,
    ) {
        let (state, hub, node_id, rx, workspace) = create_registered_node_test_state().await;
        let app = create_router((*state.as_ref()).clone());
        (app, state, hub, node_id, rx, workspace)
    }

    async fn create_permission_update_test_state() -> (
        Router,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
    ) {
        let (app, _state, _hub, node_id, rx, _workspace) =
            create_router_test_state_with_registered_node().await;
        (app, node_id, rx)
    }

    async fn create_task_submit_test_state() -> (
        Router,
        Arc<Hub>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
        TempDir,
    ) {
        let (app, _state, hub, node_id, rx, workspace) =
            create_router_test_state_with_registered_node().await;
        (app, hub, node_id, rx, workspace)
    }

    async fn create_node_token_test_state() -> Router {
        let (app, _state, _node_id, _rx) = create_router_test_state_with_security().await;
        app
    }

    async fn post_json<T: Serialize>(
        app: Router,
        path: &str,
        payload: &T,
    ) -> (StatusCode, serde_json::Value) {
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(payload).unwrap()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    async fn get_json(app: Router, path: &str) -> (StatusCode, serde_json::Value) {
        let request = Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    async fn get_text(app: Router, path: &str) -> (StatusCode, String, Option<String>) {
        let request = Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        (status, text, content_type)
    }

    async fn create_pending_approval(
        state: &Arc<WebState>,
        node_id: &uhorse_protocol::NodeId,
        request_id: &str,
    ) -> ApprovalRequest {
        let security_manager = state.hub.security_manager().unwrap();
        let approval_id = security_manager
            .operation_approver()
            .request_approval(
                node_id,
                "system_command",
                uhorse_security::ApprovalLevel::Single,
                serde_json::json!({
                    "node_id": node_id.as_str(),
                    "request_id": request_id,
                    "task_id": "task-approval-web",
                }),
            )
            .await
            .unwrap();

        security_manager
            .operation_approver()
            .get_request(&approval_id)
            .await
            .unwrap()
            .unwrap()
    }

    #[test]
    fn test_web_config_default() {
        let config = WebConfig::default();
        assert_eq!(config.port, 3000);
        assert!(config.enable_cors);
    }

    #[tokio::test]
    async fn test_health_check_returns_current_health_payload() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new(Arc::new(hub), None, None);
        let (status, body) = get_json(create_router(state), "/api/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], json!("healthy"));
        assert_eq!(body["version"], json!(env!("CARGO_PKG_VERSION")));
    }

    #[tokio::test]
    async fn test_metrics_endpoint_returns_prometheus_payload() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new(Arc::new(hub), None, None);
        let (status, body, content_type) = get_text(create_router(state), "/metrics").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            content_type.as_deref(),
            Some("text/plain; version=0.0.4; charset=utf-8")
        );
        assert!(body.contains("# HELP uhorse_messages_received_total"));
        assert!(body.contains("# TYPE uhorse_active_sessions gauge"));
        assert!(body.contains("uhorse_api_requests_total 0"));
        assert!(body.contains("uhorse_websocket_connections 0"));
        assert!(body.contains("# HELP uhorse_hub_uptime_seconds"));
        assert!(body.contains("uhorse_hub_nodes_total 0"));
        assert!(body.contains("uhorse_hub_tasks_failed 0"));
    }

    #[tokio::test]
    async fn test_api_metrics_middleware_tracks_success_and_error_requests() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let app = create_router(WebState::new(Arc::new(hub), None, None));

        let (health_status, _) = get_json(app.clone(), "/api/health").await;
        let (missing_status, _, _) = get_text(app.clone(), "/api/does-not-exist").await;
        let (metrics_status, body, _) = get_text(app, "/metrics").await;

        assert_eq!(health_status, StatusCode::OK);
        assert_eq!(missing_status, StatusCode::NOT_FOUND);
        assert_eq!(metrics_status, StatusCode::OK);
        assert!(body.contains("uhorse_api_requests_total 2"));
        assert!(body.contains("uhorse_api_errors_total 1"));
    }

    #[test]
    fn test_parse_planned_command_accepts_workspace_file_command() {
        let workspace = "/tmp/workspace";
        let response = format!(
            r#"{{"command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}}}}"#,
            workspace
        );

        let command = parse_planned_command(&response, workspace).unwrap();
        match command {
            Command::File(FileCommand::Exists { path }) => {
                assert_eq!(path, "/tmp/workspace/Cargo.toml");
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_planned_command_rejects_invalid_json() {
        let error = parse_planned_command("not-json", "/tmp/workspace").unwrap_err();
        assert!(error.to_string().contains("无效 JSON"));
    }

    #[test]
    fn test_parse_planned_command_rejects_path_outside_workspace() {
        let response = r#"{"command":{"type":"file","action":"exists","path":"/etc/passwd"}}"#;
        let error = parse_planned_command(response, "/tmp/workspace").unwrap_err();
        assert!(error.to_string().contains("workspace"));
    }

    #[test]
    fn test_parse_planned_command_rejects_dangerous_git_shell() {
        let response = r#"{"command":{"type":"shell","command":"git","args":["reset","--hard"],"cwd":"/tmp/workspace","env":{},"timeout":30,"capture_stderr":true}}"#;
        let error = parse_planned_command(response, "/tmp/workspace").unwrap_err();
        assert!(error.to_string().contains("危险 git"));
    }

    #[tokio::test]
    async fn test_summarize_task_result_or_fallback_falls_back_when_llm_fails() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(
            Arc::new(hub),
            None,
            Some(Arc::new(FailingLlmClient)),
        ));
        let completed_task = CompletedTask {
            task_id: TaskId::from_string("task-fallback"),
            command: Command::File(FileCommand::Exists {
                path: "/tmp/workspace/file.txt".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("user-1"),
                uhorse_protocol::SessionId::from_string("session-1"),
                "dingtalk",
            )
            .with_intent("检查文件是否存在"),
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            priority: Priority::Normal,
            result: CommandResult::success(CommandOutput::text("raw output")),
        };

        let reply = summarize_task_result_or_fallback(&state, &completed_task).await;
        assert_eq!(reply, "raw output");
    }

    #[tokio::test]
    async fn test_summarize_task_result_or_fallback_uses_llm_summary() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: "总结结果".to_string(),
            })),
        ));
        let completed_task = CompletedTask {
            task_id: TaskId::from_string("task-summary"),
            command: Command::File(FileCommand::Exists {
                path: "/tmp/workspace/file.txt".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("user-1"),
                uhorse_protocol::SessionId::from_string("session-1"),
                "dingtalk",
            )
            .with_intent("检查文件是否存在"),
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            priority: Priority::Normal,
            result: CommandResult::success(CommandOutput::text("raw output")),
        };

        let reply = summarize_task_result_or_fallback(&state, &completed_task).await;
        assert_eq!(reply, "总结结果");
    }

    #[tokio::test]
    async fn test_plan_dingtalk_command_uses_llm_output() {
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: format!(
                    r#"{{"command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}}}}"#,
                    workspace_root
                ),
            })),
        ));

        let session_key = SessionKey::new("dingtalk", "user-1");
        let decision = plan_dingtalk_command(
            &state,
            "检查 Cargo.toml 是否存在",
            &workspace_root,
            "main",
            &session_key,
        )
        .await
        .unwrap();

        match decision {
            AgentDecision::ExecuteCommand {
                command: Command::File(FileCommand::Exists { path }),
            } => {
                assert_eq!(path, format!("{}/Cargo.toml", workspace_root));
            }
            other => panic!("unexpected decision: {:?}", other),
        }
    }

    #[test]
    fn test_build_dingtalk_session_key_prefers_sender_user_and_corp() {
        let session_key = build_dingtalk_session_key(
            "fallback-user",
            Some("actual-user"),
            Some("staff-1"),
            Some("corp-1"),
        );

        assert_eq!(session_key.as_str(), "dingtalk:actual-user:corp-1");
    }

    #[tokio::test]
    async fn test_persist_session_state_stores_agent_metadata() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::with_team("dingtalk", "user-1", "corp-1");
        let task_id = TaskId::from_string("task-42");

        persist_session_state(
            &state,
            &session_key,
            "main",
            "conv-1",
            Some("user-1"),
            Some("staff-1"),
            Some(&task_id),
        )
        .await;

        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(session_state.message_count, 1);
        assert_eq!(
            session_state
                .metadata
                .get("current_agent")
                .map(String::as_str),
            Some("main")
        );
        assert_eq!(
            session_state
                .metadata
                .get("conversation_id")
                .map(String::as_str),
            Some("conv-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("sender_user_id")
                .map(String::as_str),
            Some("user-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("sender_staff_id")
                .map(String::as_str),
            Some("staff-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("last_task_id")
                .map(String::as_str),
            Some(task_id.as_str())
        );
    }

    #[tokio::test]
    async fn test_collect_agent_planning_context_includes_scope_and_memory() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::new("dingtalk", "user-ctx");
        let scope = runtime.agent_manager.get_scope("main").unwrap();

        tokio::fs::write(scope.workspace_dir().join("MEMORY.md"), "global facts")
            .await
            .unwrap();
        runtime
            .memory_store
            .store_message(
                &CoreSessionId::from_string(session_key.as_str()),
                "hello",
                "world",
            )
            .await
            .unwrap();

        let context = collect_agent_planning_context(&state, "main", &session_key).await;

        assert!(context.contains("Agent Workspace Context"));
        assert!(context.contains("MEMORY.md"));
        assert!(context.contains("global facts"));
        assert!(context.contains("Session Memory Context"));
        assert!(context.contains("**User:** hello"));
        assert!(context.contains("**Assistant:** world"));
    }

    #[tokio::test]
    async fn test_init_default_agent_runtime_binds_scope_to_agent() {
        let runtime = create_test_runtime().await;
        let agent = runtime.agents.get("main").unwrap();
        let scope = agent.scope().expect("main agent scope");

        assert_eq!(scope.config().agent_id, "main");
        assert_eq!(scope.workspace_dir(), agent.workspace_dir());
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_returns_direct_reply_for_plain_text() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: "直接回复用户".to_string(),
            })),
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-direct");

        let decision = decide_dingtalk_action(&state, "你好", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::DirectReply { text } if text == "直接回复用户"
        ));
    }

    #[tokio::test]
    async fn test_persist_task_result_memory_updates_history_and_today_memory() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::new("dingtalk", "user-memory");
        let task_id = TaskId::from_string("task-memory");

        persist_session_state(
            &state,
            &session_key,
            "main",
            "conv-memory",
            Some("user-memory"),
            None,
            None,
        )
        .await;

        let completed_task = CompletedTask {
            task_id: task_id.clone(),
            command: Command::File(FileCommand::Exists {
                path: "/tmp/workspace/README.md".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("user-memory"),
                uhorse_protocol::SessionId::from_string(session_key.as_str()),
                "dingtalk",
            )
            .with_intent("检查 README")
            .with_env("agent_id", "main"),
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            priority: Priority::Normal,
            result: CommandResult::success(CommandOutput::text("done")),
        };

        persist_task_result_memory(&state, &completed_task, "已完成").await;

        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let history = tokio::fs::read_to_string(
            scope
                .workspace_dir()
                .join("sessions")
                .join(session_key.as_str())
                .join("history.md"),
        )
        .await
        .unwrap();
        assert!(history.contains("**User:** 检查 README"));
        assert!(history.contains("**Assistant:** 已完成"));

        let today_memory = tokio::fs::read_to_string(scope.today_memory_file())
            .await
            .unwrap();
        assert!(today_memory.contains("**User:** 检查 README"));
        assert!(today_memory.contains("**Assistant:** 已完成"));

        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session_state
                .metadata
                .get("last_task_id")
                .map(String::as_str),
            Some(task_id.as_str())
        );
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_dispatches_assignment_and_persists_session_state() {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: format!(
                    r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}}}}"#,
                    workspace_root
                ),
            })),
            runtime.clone(),
        ));

        let node_id = uhorse_protocol::NodeId::from_string("node-submit");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id.clone(),
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                name: "workspace".to_string(),
                path: workspace_root.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();

        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Text("检查 Cargo.toml 是否存在".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-submit".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        submit_dingtalk_task(&state, inbound).await.unwrap();

        let assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let (task_id, context) = match assignment {
            uhorse_protocol::HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                match command {
                    Command::File(FileCommand::Exists { path }) => {
                        assert_eq!(path, format!("{}/Cargo.toml", workspace_root));
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                (task_id, context)
            }
            other => panic!("unexpected message: {:?}", other),
        };

        assert_eq!(context.session_id.as_str(), "dingtalk:actual-user:corp-1");
        assert_eq!(context.intent.as_deref(), Some("检查 Cargo.toml 是否存在"));
        assert_eq!(
            context.env.get("agent_id").map(String::as_str),
            Some("main")
        );
        assert_eq!(
            context.env.get("conversation_id").map(String::as_str),
            Some("conv-submit")
        );

        let status = hub.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Running);
        assert_eq!(status.node_id.as_ref(), Some(&node_id));

        let routes = state.dingtalk_routes.read().await;
        let route = routes.get(&task_id).unwrap();
        assert_eq!(route.conversation_id, "conv-submit");
        assert_eq!(route.sender_user_id.as_deref(), Some("actual-user"));
        assert_eq!(route.sender_staff_id.as_deref(), Some("staff-1"));
        assert_eq!(route.robot_code.as_deref(), Some("robot-1"));
        drop(routes);

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session_state.message_count, 1);
        assert_eq!(
            session_state
                .metadata
                .get("current_agent")
                .map(String::as_str),
            Some("main")
        );
        assert_eq!(
            session_state
                .metadata
                .get("conversation_id")
                .map(String::as_str),
            Some("conv-submit")
        );
        assert_eq!(
            session_state
                .metadata
                .get("sender_user_id")
                .map(String::as_str),
            Some("actual-user")
        );
        assert_eq!(
            session_state
                .metadata
                .get("sender_staff_id")
                .map(String::as_str),
            Some("staff-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("last_task_id")
                .map(String::as_str),
            Some(task_id.as_str())
        );
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_direct_reply_does_not_dispatch_task() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"direct_reply","text":"直接答复"}"#.to_string(),
            })),
            runtime.clone(),
        ));

        let node_id = uhorse_protocol::NodeId::from_string("node-direct");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                name: "workspace".to_string(),
                path: tempdir().unwrap().path().to_string_lossy().to_string(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();

        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Text("你好".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-direct".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        submit_dingtalk_task(&state, inbound).await.unwrap();

        let assignment =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        assert!(assignment.is_err());
        assert!(state.dingtalk_routes.read().await.is_empty());

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** 你好"));
        assert!(history.contains("**Assistant:** 直接答复"));
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_executes_local_skill_and_persists_memory() {
        let (runtime, state) = create_test_runtime_with_skill(
            "echo",
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "import os; print(os.environ['SKILL_INPUT'])"]
"#,
            r#"{"type":"execute_skill","skill_name":"echo","input":"skill output"}"#,
        )
        .await;

        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Text("运行本地技能".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-skill".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        submit_dingtalk_task(&state, inbound).await.unwrap();

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** 运行本地技能"));
        assert!(history.contains("**Assistant:** skill output"));
    }

    #[tokio::test]
    async fn test_execute_local_skill_returns_error_for_disabled_skill() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "disabled-skill",
            r#"enabled = false
timeout = 5
executable = "python3"
args = ["-c", "print('never')"]
"#,
            r#"{"type":"direct_reply","text":"unused"}"#,
        )
        .await;

        let error = execute_local_skill(&state, "disabled-skill", "input")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("disabled"));
    }

    #[tokio::test]
    async fn test_execute_local_skill_returns_error_for_stderr_output() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "stderr-skill",
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "import sys; sys.stderr.write('boom\\n')"]
"#,
            r#"{"type":"direct_reply","text":"unused"}"#,
        )
        .await;

        let error = execute_local_skill(&state, "stderr-skill", "input")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("boom"));
    }

    #[tokio::test]
    async fn test_execute_local_skill_returns_error_for_timeout() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "timeout-skill",
            r#"enabled = true
timeout = 1
executable = "python3"
args = ["-c", "import time; time.sleep(2); print('late')"]
"#,
            r#"{"type":"direct_reply","text":"unused"}"#,
        )
        .await;

        let error = execute_local_skill(&state, "timeout-skill", "input")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_execute_local_skill_pretty_prints_json_stdout() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "json-skill",
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "print('{\"ok\":true,\"message\":\"done\"}')"]
"#,
            r#"{"type":"direct_reply","text":"unused"}"#,
        )
        .await;

        let output = execute_local_skill(&state, "json-skill", "input")
            .await
            .unwrap();
        assert!(output.contains("\"ok\": true"));
        assert!(output.contains("\"message\": \"done\""));
    }

    #[tokio::test]
    async fn test_list_runtime_skills_returns_loaded_registry() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "echo",
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "import os; print(os.environ['SKILL_INPUT'])"]
"#,
            r#"{"type":"direct_reply","text":"unused"}"#,
        )
        .await;

        let Json(response) = list_runtime_skills(State(state)).await;
        let skills = response.data.unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "echo");
        assert_eq!(skills[0].execution_mode, "process");
    }

    #[tokio::test]
    async fn test_list_runtime_sessions_and_messages_return_runtime_state() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::new("dingtalk", "user-api");

        persist_session_state(
            &state,
            &session_key,
            "main",
            "conv-api",
            Some("user-api"),
            Some("staff-api"),
            None,
        )
        .await;
        runtime
            .memory_store
            .store_message(
                &CoreSessionId::from_string(session_key.as_str()),
                "hello",
                "world",
            )
            .await
            .unwrap();

        let Json(session_list_response) = list_runtime_sessions(State(state.clone())).await;
        let sessions = session_list_response.data.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_key.as_str());
        assert_eq!(sessions[0].agent_id.as_deref(), Some("main"));

        let (_, Json(messages_response)) =
            get_runtime_session_messages(State(state), Path(session_key.as_str().to_string()))
                .await;
        let messages = messages_response.data.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].user_message, "hello");
        assert_eq!(messages[0].assistant_message, "world");
    }

    #[tokio::test]
    async fn test_list_approvals_returns_pending_requests() {
        let (state, node_id, _rx) = create_security_test_state().await;
        let approval = create_pending_approval(&state, &node_id, "request-list").await;

        let (status, Json(response)) = list_approvals(State(state)).await;
        let approvals = response.data.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(approvals.len(), 1);
        assert_eq!(approvals[0].id, approval.id);
        assert_eq!(
            approvals[0].status,
            uhorse_security::ApprovalStatus::Pending
        );
    }

    #[tokio::test]
    async fn test_get_approval_returns_existing_request() {
        let (state, node_id, _rx) = create_security_test_state().await;
        let approval = create_pending_approval(&state, &node_id, "request-get").await;

        let (status, Json(response)) = get_approval(State(state), Path(approval.id.clone())).await;
        let returned = response.data.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(returned.id, approval.id);
        assert_eq!(
            returned.metadata.get("request_id"),
            Some(&serde_json::json!("request-get"))
        );
    }

    #[tokio::test]
    async fn test_approve_approval_updates_status_and_notifies_node() {
        let (state, node_id, mut rx) = create_security_test_state().await;
        let approval = create_pending_approval(&state, &node_id, "request-approve").await;

        let (status, Json(response)) = approve_approval(
            State(state.clone()),
            Path(approval.id.clone()),
            Json(ApprovalDecisionPayload {
                responder: "admin".to_string(),
                reason: Some("looks good".to_string()),
            }),
        )
        .await;
        let updated = response.data.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated.status, uhorse_security::ApprovalStatus::Approved);

        let message = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match message {
            HubToNode::ApprovalResponse {
                request_id,
                approved,
                responder,
                reason,
                ..
            } => {
                assert_eq!(request_id, "request-approve");
                assert!(approved);
                assert_eq!(responder, "admin");
                assert_eq!(reason.as_deref(), Some("looks good"));
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reject_approval_updates_status_and_notifies_node() {
        let (state, node_id, mut rx) = create_security_test_state().await;
        let approval = create_pending_approval(&state, &node_id, "request-reject").await;

        let (status, Json(response)) = reject_approval(
            State(state.clone()),
            Path(approval.id.clone()),
            Json(ApprovalDecisionPayload {
                responder: "admin".to_string(),
                reason: Some("too risky".to_string()),
            }),
        )
        .await;
        let updated = response.data.unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated.status, uhorse_security::ApprovalStatus::Rejected);

        let message = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match message {
            HubToNode::ApprovalResponse {
                request_id,
                approved,
                responder,
                reason,
                ..
            } => {
                assert_eq!(request_id, "request-reject");
                assert!(!approved);
                assert_eq!(responder, "admin");
                assert_eq!(reason.as_deref(), Some("too risky"));
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_issue_node_token_api_returns_token_pair() {
        let app = create_node_token_test_state().await;

        let (status, body) = post_json(
            app,
            "/api/node-auth/token",
            &json!({
                "node_id": "node-api-token",
                "credentials": "test-credentials"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["node_id"], json!("node-api-token"));
        assert!(body["data"]["access_token"].as_str().unwrap().len() > 10);
        assert!(body["data"]["refresh_token"].as_str().unwrap().len() > 10);
        assert!(body["data"]["expires_at"].as_str().unwrap().contains('T'));
    }

    #[tokio::test]
    async fn test_submit_task_api_dispatches_assignment_and_persists_task_status() {
        let (app, hub, node_id, mut rx, workspace) = create_task_submit_test_state().await;
        let target_path = workspace.path().join("Cargo.toml");

        let (status, body) = post_json(
            app,
            "/api/tasks",
            &json!({
                "command": {
                    "type": "file",
                    "action": "exists",
                    "path": target_path.to_string_lossy().to_string()
                },
                "user_id": "api-user",
                "session_id": "api-session",
                "channel": "api",
                "intent": "check file",
                "env": {
                    "source": "web-test"
                },
                "priority": "normal",
                "workspace_hint": workspace.path().to_string_lossy().to_string(),
                "required_tags": []
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["success"], json!(true));
        let task_id = body["data"]["task_id"].as_str().unwrap().to_string();

        let assignment = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match assignment {
            HubToNode::TaskAssignment {
                task_id: assigned_task_id,
                command,
                context,
                ..
            } => {
                assert_eq!(assigned_task_id.as_str(), task_id);
                assert_eq!(context.user_id.as_str(), "api-user");
                assert_eq!(context.session_id.as_str(), "api-session");
                assert_eq!(context.channel, "api");
                assert_eq!(context.intent.as_deref(), Some("check file"));
                assert_eq!(
                    context.env.get("source").map(String::as_str),
                    Some("web-test")
                );
                match command {
                    Command::File(FileCommand::Exists { path }) => {
                        assert_eq!(path, target_path.to_string_lossy().to_string());
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
            }
            other => panic!("unexpected message: {:?}", other),
        }

        let status = hub
            .get_task_status(&TaskId::from_string(task_id))
            .await
            .unwrap();
        assert_eq!(status.status, TaskStatus::Running);
        assert_eq!(status.node_id.as_ref(), Some(&node_id));
    }

    #[tokio::test]
    async fn test_update_node_permissions_api_sends_permission_update_to_node() {
        let (app, node_id, mut rx) = create_permission_update_test_state().await;

        let (status, body) = post_json(
            app,
            &format!("/api/nodes/{}/permissions", node_id.as_str()),
            &json!({
                "rules": [
                    {
                        "id": "approval-shell",
                        "name": "Require shell approval",
                        "resource": {
                            "type": "prefix",
                            "prefix": "/tmp"
                        },
                        "actions": ["execute"],
                        "conditions": [],
                        "require_approval": true,
                        "enabled": true
                    }
                ]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"], json!("Permission update sent"));

        let message = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match message {
            HubToNode::PermissionUpdate { rules, .. } => {
                assert_eq!(rules.len(), 1);
                assert_eq!(rules[0].id, "approval-shell");
                assert_eq!(rules[0].name, "Require shell approval");
                assert!(rules[0].require_approval);
                assert_eq!(rules[0].actions.len(), 1);
                assert!(matches!(rules[0].actions[0], Action::Execute));
                match &rules[0].resource {
                    ProtocolResourcePattern::Prefix { prefix } => assert_eq!(prefix, "/tmp"),
                    other => panic!("unexpected resource: {:?}", other),
                }
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[test]
    fn test_validate_shell_command_allows_workspace_cwd_none() {
        let command = ShellCommand::new("pwd");
        assert!(validate_shell_command(&command, "/tmp/workspace").is_ok());
    }

    #[test]
    fn test_format_task_result_message_failure() {
        let result = CommandResult::failure(ExecutionError::execution_failed("boom"));
        assert_eq!(format_task_result_message(&result), "执行失败：boom");
    }

    #[tokio::test]
    async fn test_get_task_returns_actual_command_type_and_priority_for_completed_task() {
        let (app, hub, _node_id, mut rx, workspace) = create_task_submit_test_state().await;
        let path = workspace
            .path()
            .join("Cargo.toml")
            .to_string_lossy()
            .to_string();

        let (status, body) = post_json(
            app,
            "/api/tasks",
            &json!({
                "command": {
                    "type": "file",
                    "action": "exists",
                    "path": path
                },
                "user_id": "status-user",
                "session_id": "status-session",
                "channel": "api",
                "priority": "high",
                "required_tags": []
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        let task_id = body["data"]["task_id"].as_str().unwrap().to_string();

        let _assignment = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        let (status, body) = get_json(
            create_router(WebState::new(hub.clone(), None, None)),
            &format!("/api/tasks/{}", task_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["command_type"], json!("file"));
        assert_eq!(body["data"]["priority"], json!("high"));
        assert_eq!(body["data"]["status"], json!("Running"));
    }
}
