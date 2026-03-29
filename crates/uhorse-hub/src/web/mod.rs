//! Web 管理界面
//!
//! 提供 Hub 的 HTTP 管理接口

pub mod ws;

use axum::{
    extract::{MatchedPath, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use uhorse_agent::{
    scope_layer_from_scope, scope_layer_rank, AccessContext, Agent, AgentManager, AgentScope,
    AgentScopeConfig, LayeredMemoryStore, LayeredSkillEntry, LayeredSkillRegistry, MemoryStore,
    SessionKey, SessionNamespace, SessionState, SkillRegistry,
};
use uhorse_channel::{dingtalk::DingTalkEvent, DingTalkChannel, DingTalkInboundMessage};
use uhorse_core::{Channel, MessageContent, SessionId as CoreSessionId};
use uhorse_llm::{ChatMessage, LLMClient};
use uhorse_observability::{HealthService, HealthStatus, MetricsCollector, MetricsExporter};
use uhorse_protocol::{
    BrowserResult, Command, CommandOutput, CommandType, FileCommand, HubToNode, MessageId,
    NodeCapabilities, PermissionRule as ProtocolPermissionRule, Priority, SessionId, TaskContext,
    TaskId, UserId,
};
use uhorse_security::ApprovalRequest;

use crate::{
    node_manager::workspace_matches_hint,
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

const DINGTALK_PROCESSING_ACK_TEXT: &str = "收到啦，正在处理，请稍等～";

#[derive(Debug, Clone, PartialEq, Eq)]
enum DingTalkReplyTarget {
    SessionWebhook {
        webhook: String,
        at_user_ids: Vec<String>,
    },
    GroupConversation {
        conversation_id: String,
    },
    DirectUser {
        user_id: String,
    },
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
    DirectReply {
        text: String,
    },
    ExecuteCommand {
        command: Command,
        workspace_path: Option<String>,
    },
    ExecuteSkill {
        skill_name: String,
        input: String,
    },
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
    source_layer: String,
    source_scope: Option<String>,
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
    source_layer: String,
    source_scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentRuntimeQuery {
    source_layer: Option<String>,
    source_scope: Option<String>,
}

#[derive(Clone)]
struct CatalogAgentEntry {
    agent: Agent,
    source_layer: &'static str,
    source_scope: Option<String>,
}

/// 分层 Agent catalog。
#[derive(Clone, Default)]
pub struct LayeredAgentCatalog {
    global: HashMap<String, Agent>,
    scoped: HashMap<String, HashMap<String, Agent>>,
}

impl LayeredAgentCatalog {
    fn new(global: HashMap<String, Agent>) -> Self {
        Self {
            global,
            scoped: HashMap::new(),
        }
    }

    fn register_scoped_catalog(
        &mut self,
        scope: impl Into<String>,
        agents: HashMap<String, Agent>,
    ) {
        let scope = scope.into();
        if agents.is_empty() {
            self.scoped.remove(&scope);
        } else {
            self.scoped.insert(scope, agents);
        }
    }

    fn contains_any(&self, agent_id: &str) -> bool {
        self.get_any(agent_id).is_some()
    }

    fn get(&self, agent_id: &str) -> Option<Agent> {
        self.get_any(agent_id)
    }

    fn get_for_scopes(&self, scopes: &[String], agent_id: &str) -> Option<Agent> {
        self.get_for_scopes_entry(scopes, agent_id)
            .map(|entry| entry.agent)
    }

    fn get_entry_by_source(
        &self,
        agent_id: &str,
        source_layer: &str,
        source_scope: Option<&str>,
    ) -> Option<CatalogAgentEntry> {
        match source_layer {
            "global" => self
                .global
                .get(agent_id)
                .cloned()
                .filter(|_| source_scope.is_none())
                .map(|agent| CatalogAgentEntry {
                    agent,
                    source_layer: "global",
                    source_scope: None,
                }),
            _ => source_scope.and_then(|scope| {
                (scope_layer_from_scope(scope) == source_layer)
                    .then_some(scope)
                    .and_then(|scope| {
                        self.scoped
                            .get(scope)
                            .and_then(|catalog| catalog.get(agent_id))
                    })
                    .cloned()
                    .map(|agent| CatalogAgentEntry {
                        agent,
                        source_layer: scope_layer_from_scope(scope),
                        source_scope: Some(scope.to_string()),
                    })
            }),
        }
    }

    fn get_for_scopes_entry(&self, scopes: &[String], agent_id: &str) -> Option<CatalogAgentEntry> {
        for scope in scopes {
            if let Some(agent) = self
                .scoped
                .get(scope)
                .and_then(|catalog| catalog.get(agent_id))
                .cloned()
            {
                return Some(CatalogAgentEntry {
                    agent,
                    source_layer: scope_layer_from_scope(scope),
                    source_scope: Some(scope.clone()),
                });
            }
        }

        self.global
            .get(agent_id)
            .cloned()
            .map(|agent| CatalogAgentEntry {
                agent,
                source_layer: "global",
                source_scope: None,
            })
    }

    fn list_all_ids(&self) -> Vec<String> {
        let mut names = self.global.keys().cloned().collect::<Vec<_>>();
        for catalog in self.scoped.values() {
            names.extend(catalog.keys().cloned());
        }
        names.sort();
        names.dedup();
        names
    }

    fn list_all_entries(&self) -> Vec<CatalogAgentEntry> {
        let mut entries = Vec::new();

        for agent in self.global.values() {
            entries.push(CatalogAgentEntry {
                agent: agent.clone(),
                source_layer: "global",
                source_scope: None,
            });
        }

        for scope in sorted_catalog_scopes(&self.scoped) {
            if let Some(catalog) = self.scoped.get(&scope) {
                let source_layer = scope_layer_from_scope(&scope);
                for agent in catalog.values() {
                    entries.push(CatalogAgentEntry {
                        agent: agent.clone(),
                        source_layer,
                        source_scope: Some(scope.clone()),
                    });
                }
            }
        }

        entries.sort_by(|left, right| {
            left.agent
                .agent_id()
                .cmp(right.agent.agent_id())
                .then_with(|| {
                    scope_layer_rank(left.source_layer).cmp(&scope_layer_rank(right.source_layer))
                })
                .then_with(|| {
                    left.source_scope
                        .as_deref()
                        .unwrap_or("")
                        .cmp(right.source_scope.as_deref().unwrap_or(""))
                })
        });
        entries
    }

    fn get_any(&self, agent_id: &str) -> Option<Agent> {
        self.get_any_entry(agent_id).map(|entry| entry.agent)
    }

    fn get_any_entry(&self, agent_id: &str) -> Option<CatalogAgentEntry> {
        for scope in sorted_catalog_scopes_by_rank(&self.scoped) {
            if let Some(agent) = self
                .scoped
                .get(&scope)
                .and_then(|catalog| catalog.get(agent_id))
            {
                return Some(CatalogAgentEntry {
                    agent: agent.clone(),
                    source_layer: scope_layer_from_scope(&scope),
                    source_scope: Some(scope),
                });
            }
        }
        self.global
            .get(agent_id)
            .cloned()
            .map(|agent| CatalogAgentEntry {
                agent,
                source_layer: "global",
                source_scope: None,
            })
    }
}

fn sorted_catalog_scopes<T>(catalog: &HashMap<String, T>) -> Vec<String> {
    let mut scopes = catalog.keys().cloned().collect::<Vec<_>>();
    scopes.sort();
    scopes
}

fn sorted_catalog_scopes_by_rank<T>(catalog: &HashMap<String, T>) -> Vec<String> {
    let mut scopes = catalog.keys().cloned().collect::<Vec<_>>();
    scopes.sort_by(|left, right| {
        scope_layer_rank(scope_layer_from_scope(left))
            .cmp(&scope_layer_rank(scope_layer_from_scope(right)))
            .then_with(|| left.cmp(right))
    });
    scopes
}

fn agent_display_name(agent_id: &str) -> String {
    if agent_id == "main" {
        return "Main Agent".to_string();
    }

    let segments = agent_id
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();

    if segments.is_empty() {
        agent_id.to_string()
    } else {
        segments.join(" ")
    }
}

async fn build_catalog_agent(
    agent_id: &str,
    workspace_dir: PathBuf,
    is_default: bool,
    description: impl Into<String>,
    system_prompt: impl Into<String>,
) -> Result<Agent, Box<dyn std::error::Error + Send + Sync>> {
    let display_name = agent_display_name(agent_id);
    let scope = AgentScope::new(AgentScopeConfig {
        agent_id: agent_id.to_string(),
        workspace_dir: workspace_dir.clone(),
        display_name: Some(display_name.clone()),
        is_default,
    })?;
    scope.init_workspace().await?;

    Ok(Agent::builder()
        .agent_id(agent_id)
        .name(display_name)
        .description(description)
        .workspace_dir(workspace_dir)
        .system_prompt(system_prompt)
        .set_default(is_default)
        .build()?
        .with_scope(scope))
}

async fn load_agent_catalog_from_root(
    root: &FsPath,
    include_main: bool,
) -> Result<HashMap<String, Agent>, Box<dyn std::error::Error + Send + Sync>> {
    let mut agents = HashMap::new();
    if !root.exists() {
        return Ok(agents);
    }

    let mut candidates = Vec::new();
    if include_main {
        let main_workspace = root.join("workspace");
        if main_workspace.is_dir() {
            candidates.push(("main".to_string(), main_workspace));
        }
    }

    let mut entries = tokio::fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(agent_id) = name.strip_prefix("workspace-") else {
            continue;
        };
        if agent_id.is_empty() {
            continue;
        }
        candidates.push((agent_id.to_string(), path));
    }

    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    for (agent_id, workspace_dir) in candidates {
        let is_default = false;
        let description = format!("{} agent", agent_display_name(&agent_id));
        let system_prompt = format!("You are the {} for uHorse.", agent_display_name(&agent_id));
        let agent = build_catalog_agent(
            &agent_id,
            workspace_dir,
            is_default,
            description,
            system_prompt,
        )
        .await?;
        agents.insert(agent_id, agent);
    }

    Ok(agents)
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
    source_layer: String,
    source_scope: Option<String>,
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
    source_layer: String,
    source_scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillRuntimeQuery {
    source_layer: Option<String>,
    source_scope: Option<String>,
}

/// Hub 侧逻辑协作工作空间视图。
#[derive(Debug, Clone, Serialize)]
pub struct CollaborationWorkspace {
    collaboration_workspace_id: String,
    scope_owner: String,
    members: Vec<String>,
    default_agent_id: Option<String>,
    visible_scope_chain: Vec<String>,
    bound_execution_workspace_id: Option<String>,
    materialization: String,
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
    namespace: Option<SessionNamespace>,
    collaboration_workspace: Option<CollaborationWorkspace>,
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
    namespace: Option<SessionNamespace>,
    collaboration_workspace: Option<CollaborationWorkspace>,
    memory_context_chain: Vec<String>,
    visibility_chain: Vec<String>,
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
    /// 分层 Agent catalog
    pub agents: Arc<LayeredAgentCatalog>,
    /// Memory 存储
    pub memory_store: Arc<LayeredMemoryStore>,
    /// 分层 Skill 注册表
    pub skills: Arc<LayeredSkillRegistry>,
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
    let memory_store = Arc::new(LayeredMemoryStore::new(
        std::env::temp_dir().join("uhorse-web-memory"),
    ));

    WebAgentRuntime {
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(LayeredAgentCatalog::default()),
        memory_store,
        skills: Arc::new(LayeredSkillRegistry::new(SkillRegistry::new())),
    }
}

/// 初始化默认 Web Agent 运行时
pub async fn init_default_agent_runtime(
    base_dir: PathBuf,
) -> Result<WebAgentRuntime, Box<dyn std::error::Error + Send + Sync>> {
    let mut agent_manager = AgentManager::new(base_dir.clone())?;

    let main_agent = build_catalog_agent(
        "main",
        base_dir.join("workspace"),
        true,
        "Hub default agent",
        "You are the default uHorse Hub agent.",
    )
    .await?;
    if let Some(scope) = main_agent.scope().cloned() {
        agent_manager.register_scope(Arc::new(scope))?;
    }

    let mut global_agents = HashMap::from([("main".to_string(), main_agent)]);
    let additional_global_agents = load_agent_catalog_from_root(&base_dir, false).await?;
    for agent in additional_global_agents.values() {
        if let Some(scope) = agent.scope().cloned() {
            agent_manager.register_scope(Arc::new(scope))?;
        }
    }
    global_agents.extend(additional_global_agents);
    let mut layered_agents = LayeredAgentCatalog::new(global_agents);

    let global_skills = SkillRegistry::from_dir(base_dir.join("skills")).await?;
    let mut layered_skills = LayeredSkillRegistry::new(global_skills);

    async fn load_scoped_runtime_dir(
        base_dir: &FsPath,
        dir_name: &str,
        layered_skills: &mut LayeredSkillRegistry,
        layered_agents: &mut LayeredAgentCatalog,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let scoped_dir = base_dir.join(dir_name);
        if !scoped_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&scoped_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(scope) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let registry = SkillRegistry::from_dir(path.join("skills")).await?;
            if !registry.is_empty() {
                layered_skills.register_scoped_registry(scope.to_string(), registry);
            }
            let catalog = load_agent_catalog_from_root(&path, true).await?;
            if !catalog.is_empty() {
                layered_agents.register_scoped_catalog(scope.to_string(), catalog);
            }
        }

        Ok(())
    }

    load_scoped_runtime_dir(
        &base_dir,
        "tenants",
        &mut layered_skills,
        &mut layered_agents,
    )
    .await?;
    load_scoped_runtime_dir(
        &base_dir,
        "enterprises",
        &mut layered_skills,
        &mut layered_agents,
    )
    .await?;
    load_scoped_runtime_dir(
        &base_dir,
        "departments",
        &mut layered_skills,
        &mut layered_agents,
    )
    .await?;
    load_scoped_runtime_dir(&base_dir, "roles", &mut layered_skills, &mut layered_agents).await?;
    load_scoped_runtime_dir(&base_dir, "users", &mut layered_skills, &mut layered_agents).await?;

    let memory = LayeredMemoryStore::new(base_dir.join("workspace"));
    memory.init_workspace().await?;

    Ok(WebAgentRuntime {
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(layered_agents),
        memory_store: Arc::new(memory),
        skills: Arc::new(layered_skills),
    })
}

fn default_agent_id(state: &WebState) -> String {
    if state.agent_runtime.agents.contains_any("main") {
        "main".to_string()
    } else {
        state
            .agent_runtime
            .agents
            .list_all_ids()
            .into_iter()
            .next()
            .unwrap_or_else(|| "main".to_string())
    }
}

fn agent_scope_for(state: &WebState, agent_id: &str) -> Option<Arc<AgentScope>> {
    state
        .agent_runtime
        .agents
        .get(agent_id)
        .and_then(|agent| agent.scope().cloned().map(Arc::new))
        .or_else(|| {
            state
                .agent_runtime
                .agent_manager
                .get_default_scope()
                .cloned()
        })
}

fn access_context_from_metadata(metadata: &HashMap<String, String>) -> Option<AccessContext> {
    let roles = metadata
        .get("namespace_roles")
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    let access_context = AccessContext {
        tenant: metadata.get("namespace_tenant").cloned(),
        enterprise: metadata.get("namespace_enterprise").cloned(),
        department: metadata.get("namespace_department").cloned(),
        roles,
    }
    .normalized();

    (access_context.tenant.is_some()
        || access_context.enterprise.is_some()
        || access_context.department.is_some()
        || !access_context.roles.is_empty())
    .then_some(access_context)
}

fn session_namespace_from_session_key(
    session_key: &SessionKey,
    access_context: Option<&AccessContext>,
) -> SessionNamespace {
    session_key.namespace_with_access_context(access_context)
}

fn session_namespace_from_metadata(
    session_key: Option<&SessionKey>,
    metadata: &HashMap<String, String>,
) -> Option<SessionNamespace> {
    let access_context = access_context_from_metadata(metadata);
    let base = if let Some(session_key) = session_key {
        session_namespace_from_session_key(session_key, access_context.as_ref())
    } else {
        let session_id = metadata.get("session_id")?;
        let session_key = SessionKey::parse(session_id).ok()?;
        session_namespace_from_session_key(&session_key, access_context.as_ref())
    };

    let mut namespace = base;
    namespace.global = metadata
        .get("namespace_global")
        .cloned()
        .unwrap_or_else(|| namespace.global.clone());
    namespace.user = metadata
        .get("namespace_user")
        .cloned()
        .unwrap_or_else(|| namespace.user.clone());
    namespace.session = metadata
        .get("namespace_session")
        .cloned()
        .unwrap_or_else(|| namespace.session.clone());
    Some(namespace)
}

fn session_namespace_for_session(
    session_key: &SessionKey,
    session_state: Option<&SessionState>,
) -> SessionNamespace {
    session_state
        .and_then(|session_state| {
            let mut metadata = session_state.metadata.clone();
            metadata.insert("session_id".to_string(), session_state.session_id.clone());
            session_namespace_from_metadata(Some(session_key), &metadata)
        })
        .unwrap_or_else(|| session_namespace_from_session_key(session_key, None))
}

fn write_namespace_metadata(metadata: &mut HashMap<String, String>, namespace: &SessionNamespace) {
    metadata.insert("namespace_global".to_string(), namespace.global.clone());
    if let Some(tenant) = &namespace.tenant {
        metadata.insert("namespace_tenant".to_string(), tenant.clone());
    } else {
        metadata.remove("namespace_tenant");
    }
    if let Some(enterprise) = &namespace.enterprise {
        metadata.insert("namespace_enterprise".to_string(), enterprise.clone());
    } else {
        metadata.remove("namespace_enterprise");
    }
    if let Some(department) = &namespace.department {
        metadata.insert("namespace_department".to_string(), department.clone());
    } else {
        metadata.remove("namespace_department");
    }
    if namespace.roles.is_empty() {
        metadata.remove("namespace_roles");
    } else if let Ok(serialized_roles) = serde_json::to_string(&namespace.roles) {
        metadata.insert("namespace_roles".to_string(), serialized_roles);
    }
    metadata.insert("namespace_user".to_string(), namespace.user.clone());
    metadata.insert("namespace_session".to_string(), namespace.session.clone());
}

fn collaboration_workspace_id_for_session(session_key: &SessionKey) -> String {
    format!("collab:session:{}", session_key.as_str())
}

fn collaboration_workspace_id_from_metadata(
    metadata: &HashMap<String, String>,
    session_key: Option<&SessionKey>,
) -> Option<String> {
    metadata
        .get("collaboration_workspace_id")
        .cloned()
        .or_else(|| session_key.map(collaboration_workspace_id_for_session))
}

fn collaboration_scope_owner_from_metadata_or_default(
    collaboration_workspace_id: &str,
    namespace: Option<&SessionNamespace>,
    metadata: &HashMap<String, String>,
) -> String {
    if collaboration_workspace_id.starts_with("collab:session:") {
        namespace
            .map(|namespace| namespace.session.clone())
            .or_else(|| metadata.get("namespace_session").cloned())
            .or_else(|| metadata.get("session_id").cloned())
            .unwrap_or_else(|| collaboration_workspace_id.to_string())
    } else if let Some(scope_owner) = metadata.get("collaboration_scope_owner") {
        scope_owner.clone()
    } else {
        namespace
            .map(|namespace| namespace.session.clone())
            .or_else(|| metadata.get("namespace_session").cloned())
            .or_else(|| metadata.get("session_id").cloned())
            .unwrap_or_else(|| collaboration_workspace_id.to_string())
    }
}

fn collaboration_materialization_from_metadata(metadata: &HashMap<String, String>) -> String {
    metadata
        .get("collaboration_materialization")
        .cloned()
        .unwrap_or_else(|| "none".to_string())
}

fn write_collaboration_workspace_runtime_metadata(
    metadata: &mut HashMap<String, String>,
    collaboration_workspace_id: &str,
    namespace: Option<&SessionNamespace>,
) {
    let scope_owner = collaboration_scope_owner_from_metadata_or_default(
        collaboration_workspace_id,
        namespace,
        metadata,
    );
    let materialization = collaboration_materialization_from_metadata(metadata);
    metadata.insert(
        "collaboration_workspace_id".to_string(),
        collaboration_workspace_id.to_string(),
    );
    metadata.insert("collaboration_scope_owner".to_string(), scope_owner);
    metadata.insert("collaboration_materialization".to_string(), materialization);
}

fn resolve_collaboration_workspace(
    session_key: Option<&SessionKey>,
    collaboration_workspace_id: Option<String>,
    namespace: Option<&SessionNamespace>,
    metadata: &HashMap<String, String>,
    default_agent_id: Option<String>,
) -> Option<CollaborationWorkspace> {
    collaboration_workspace_from_parts(
        collaboration_workspace_id
            .or_else(|| collaboration_workspace_id_from_metadata(metadata, session_key)),
        namespace,
        metadata,
        default_agent_id,
    )
}

fn build_collaboration_workspace(
    session_key: Option<&SessionKey>,
    collaboration_workspace_id: Option<String>,
    namespace: Option<&SessionNamespace>,
    metadata: &HashMap<String, String>,
    default_agent_id: Option<String>,
) -> Option<CollaborationWorkspace> {
    resolve_collaboration_workspace(
        session_key,
        collaboration_workspace_id,
        namespace,
        metadata,
        default_agent_id,
    )
}

fn render_collaboration_workspace_context(
    collaboration_workspace: &CollaborationWorkspace,
) -> String {
    format!(
        "--- Collaboration Workspace Context ---\n- collaboration_workspace_id: {}\n- scope_owner: {}\n- default_agent_id: {}\n- bound_execution_workspace_id: {}\n- materialization: {}\n- members: {}\n- visible_scope_chain: {}\n- note: 这是 Hub 侧逻辑协作空间，不是 Node 实际执行目录。",
        collaboration_workspace.collaboration_workspace_id,
        collaboration_workspace.scope_owner,
        collaboration_workspace.default_agent_id.as_deref().unwrap_or("-"),
        collaboration_workspace
            .bound_execution_workspace_id
            .as_deref()
            .unwrap_or("-"),
        collaboration_workspace.materialization,
        if collaboration_workspace.members.is_empty() {
            "-".to_string()
        } else {
            collaboration_workspace.members.join(", ")
        },
        if collaboration_workspace.visible_scope_chain.is_empty() {
            "-".to_string()
        } else {
            collaboration_workspace.visible_scope_chain.join(" -> ")
        },
    )
}

fn populate_task_context_runtime_metadata(
    context: &mut TaskContext,
    session_key: Option<&SessionKey>,
    source_metadata: Option<&HashMap<String, String>>,
) {
    let namespace = source_metadata
        .and_then(|metadata| {
            session_key.and_then(|session_key| {
                session_namespace_from_metadata(Some(session_key), metadata)
            })
        })
        .or_else(|| task_context_namespace(session_key, context));
    if let Some(namespace) = namespace.as_ref() {
        write_namespace_metadata(&mut context.env, namespace);
    }

    if let Some(execution_workspace_id) = context.execution_workspace_id.clone() {
        context
            .env
            .insert("execution_workspace_id".to_string(), execution_workspace_id);
    }

    let collaboration_workspace_id = context
        .collaboration_workspace_id
        .clone()
        .or_else(|| {
            source_metadata.and_then(|metadata| {
                collaboration_workspace_id_from_metadata(metadata, session_key)
            })
        })
        .or_else(|| collaboration_workspace_id_from_metadata(&context.env, session_key));
    if let Some(collaboration_workspace_id) = collaboration_workspace_id {
        context.collaboration_workspace_id = Some(collaboration_workspace_id.clone());
        write_collaboration_workspace_runtime_metadata(
            &mut context.env,
            &collaboration_workspace_id,
            namespace.as_ref(),
        );
    }
}

fn collaboration_workspace_from_parts(
    collaboration_workspace_id: Option<String>,
    namespace: Option<&SessionNamespace>,
    metadata: &HashMap<String, String>,
    default_agent_id: Option<String>,
) -> Option<CollaborationWorkspace> {
    let collaboration_workspace_id = collaboration_workspace_id?;
    let visible_scope_chain = namespace
        .map(SessionNamespace::visibility_chain)
        .unwrap_or_default();
    let scope_owner = collaboration_scope_owner_from_metadata_or_default(
        &collaboration_workspace_id,
        namespace,
        metadata,
    );
    let mut members = Vec::new();
    if let Some(sender_user_id) = metadata.get("sender_user_id") {
        members.push(sender_user_id.clone());
    }
    if let Some(sender_staff_id) = metadata.get("sender_staff_id") {
        if !members.contains(sender_staff_id) {
            members.push(sender_staff_id.clone());
        }
    }
    if let Some(user_scope) = namespace.map(|namespace| namespace.user.clone()) {
        if !members.contains(&user_scope) {
            members.push(user_scope);
        }
    }

    Some(CollaborationWorkspace {
        collaboration_workspace_id,
        scope_owner,
        members,
        default_agent_id,
        visible_scope_chain,
        bound_execution_workspace_id: metadata.get("execution_workspace_id").cloned(),
        materialization: collaboration_materialization_from_metadata(metadata),
    })
}

fn agent_scope_from_entry(entry: &CatalogAgentEntry) -> Option<Arc<AgentScope>> {
    entry.agent.scope().cloned().map(Arc::new)
}

fn agent_entry_from_source_metadata(
    state: &WebState,
    agent_id: &str,
    source_layer: Option<&str>,
    source_scope: Option<&str>,
) -> Option<CatalogAgentEntry> {
    source_layer.and_then(|layer| {
        state
            .agent_runtime
            .agents
            .get_entry_by_source(agent_id, layer, source_scope)
    })
}

fn session_state_agent_entry(
    state: &WebState,
    session_state: &SessionState,
) -> Option<CatalogAgentEntry> {
    let agent_id = session_state.metadata.get("current_agent")?;
    agent_entry_from_source_metadata(
        state,
        agent_id,
        session_state
            .metadata
            .get("agent_source_layer")
            .map(String::as_str),
        session_state
            .metadata
            .get("agent_source_scope")
            .map(String::as_str),
    )
}

fn task_context_namespace(
    session_key: Option<&SessionKey>,
    context: &TaskContext,
) -> Option<SessionNamespace> {
    if let Some(session_key) = session_key {
        return session_namespace_from_metadata(Some(session_key), &context.env)
            .or_else(|| Some(session_namespace_from_session_key(session_key, None)));
    }

    let mut metadata = context.env.clone();
    metadata.insert(
        "session_id".to_string(),
        context.session_id.as_str().to_string(),
    );
    session_namespace_from_metadata(None, &metadata)
}

fn task_context_agent_entry(
    state: &WebState,
    session_key: Option<&SessionKey>,
    context: &TaskContext,
) -> Option<CatalogAgentEntry> {
    let agent_id = context.env.get("agent_id").map(String::as_str)?;

    if let Some(entry) = agent_entry_from_source_metadata(
        state,
        agent_id,
        context.env.get("agent_source_layer").map(String::as_str),
        context.env.get("agent_source_scope").map(String::as_str),
    ) {
        return Some(entry);
    }

    if let Some(namespace) = task_context_namespace(session_key, context) {
        if let Some(entry) = state
            .agent_runtime
            .agents
            .get_for_scopes_entry(&namespace.visibility_chain(), agent_id)
        {
            return Some(entry);
        }
    }

    state.agent_runtime.agents.get_any_entry(agent_id)
}

async fn resolve_agent_entry_for_session(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    requested_agent_id: Option<&str>,
) -> Option<CatalogAgentEntry> {
    let session_state = load_session_state_for_session(state, session_key).await;
    let namespace = session_namespace_for_session(session_key, session_state.as_ref());
    let visibility_chain = namespace.visibility_chain();

    if let Some(session_state) = session_state {
        if let Some(entry) =
            session_state_agent_entry(state.as_ref(), &session_state).filter(|entry| {
                requested_agent_id
                    .map(|agent_id| entry.agent.agent_id() == agent_id)
                    .unwrap_or(true)
            })
        {
            return Some(entry);
        }

        if let Some(agent_id) = session_state
            .metadata
            .get("current_agent")
            .map(String::as_str)
            .filter(|agent_id| {
                requested_agent_id
                    .map(|requested| requested == *agent_id)
                    .unwrap_or(true)
            })
        {
            if let Some(entry) = state
                .agent_runtime
                .agents
                .get_for_scopes_entry(&visibility_chain, agent_id)
                .or_else(|| state.agent_runtime.agents.get_any_entry(agent_id))
            {
                return Some(entry);
            }
        }
    }

    if let Some(agent_id) = requested_agent_id {
        return state
            .agent_runtime
            .agents
            .get_for_scopes_entry(&visibility_chain, agent_id)
            .or_else(|| state.agent_runtime.agents.get_any_entry(agent_id));
    }

    let fallback = default_agent_id(state.as_ref());
    state
        .agent_runtime
        .agents
        .get_for_scopes_entry(&visibility_chain, &fallback)
        .or_else(|| state.agent_runtime.agents.get_any_entry(&fallback))
}

async fn resolve_agent_scope_for_session(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    requested_agent_id: Option<&str>,
) -> Option<Arc<AgentScope>> {
    resolve_agent_entry_for_session(state, session_key, requested_agent_id)
        .await
        .and_then(|entry| agent_scope_from_entry(&entry))
}

async fn load_session_state_for_session(
    state: &Arc<WebState>,
    session_key: &SessionKey,
) -> Option<SessionState> {
    let mut latest = None;
    let mut scanned_scope_dirs = HashSet::new();

    for entry in state.agent_runtime.agents.list_all_entries() {
        let Some(scope) = entry.agent.scope().cloned() else {
            continue;
        };
        let scope_dir = scope.agents_dir().to_string_lossy().to_string();
        if !scanned_scope_dirs.insert(scope_dir) {
            continue;
        }
        let Ok(Some(session_state)) = scope.load_session_state(&session_key.as_str()).await else {
            continue;
        };
        let should_replace = latest
            .as_ref()
            .map(|existing: &SessionState| session_state.last_active > existing.last_active)
            .unwrap_or(true);
        if should_replace {
            latest = Some(session_state);
        }
    }

    if let Some(session_state) = latest {
        return Some(session_state);
    }

    let scope = state
        .agent_runtime
        .agent_manager
        .get_default_scope()
        .cloned()?;
    scope
        .load_session_state(&session_key.as_str())
        .await
        .ok()
        .flatten()
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
    resolve_agent_entry_for_session(state, session_key, None)
        .await
        .map(|entry| entry.agent.agent_id().to_string())
        .unwrap_or_else(|| default_agent_id(state.as_ref()))
}

async fn collect_agent_planning_context(
    state: &Arc<WebState>,
    agent_id: &str,
    session_key: &SessionKey,
) -> String {
    let core_session_id = CoreSessionId::from_string(session_key.as_str());
    let mut sections = Vec::new();
    let session_state = load_session_state_for_session(state, session_key).await;
    let namespace = session_namespace_for_session(session_key, session_state.as_ref());
    let tenant = namespace.tenant.as_deref().unwrap_or("-");
    let enterprise = namespace.enterprise.as_deref().unwrap_or("-");
    let department = namespace.department.as_deref().unwrap_or("-");
    let roles = if namespace.roles.is_empty() {
        "-".to_string()
    } else {
        namespace.roles.join(", ")
    };
    let empty_metadata = HashMap::new();
    let collaboration_metadata = session_state
        .as_ref()
        .map(|session_state| &session_state.metadata)
        .unwrap_or(&empty_metadata);
    let collaboration_workspace = build_collaboration_workspace(
        Some(session_key),
        None,
        Some(&namespace),
        collaboration_metadata,
        Some(agent_id.to_string()),
    );
    sections.push(format!(
        "--- Session Namespace ---\n- global: {}\n- tenant: {}\n- enterprise: {}\n- department: {}\n- roles: {}\n- user: {}\n- session: {}\n- memory_context_chain: {}\n- visibility_chain: {}",
        namespace.global,
        tenant,
        enterprise,
        department,
        roles,
        namespace.user,
        namespace.session,
        namespace.memory_context_chain().join(" -> "),
        namespace.visibility_chain().join(" -> "),
    ));

    if let Some(collaboration_workspace) = collaboration_workspace {
        sections.push(render_collaboration_workspace_context(
            &collaboration_workspace,
        ));
    }

    if let Some(scope) = resolve_agent_entry_for_session(state, session_key, Some(agent_id))
        .await
        .and_then(|entry| agent_scope_from_entry(&entry))
    {
        let injected_files = scope
            .get_injected_files(&core_session_id, None)
            .await
            .unwrap_or_default();
        if !injected_files.is_empty() {
            let mut block = String::from("--- Agent Workspace Context ---\n");
            for (name, content) in injected_files {
                block.push_str(&format!("\n## {}\n{}\n", name, content));
            }
            block.push_str("\n注：此上下文仅供 Agent 决策参考，不等于 Node 实际执行目录。\n");
            sections.push(block);
        }
    }

    let memory_context = state
        .agent_runtime
        .memory_store
        .get_context_for_namespace(&core_session_id, &namespace)
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

fn collect_online_workspace_roots(nodes: &[crate::node_manager::NodeInfo]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut roots = Vec::new();

    for node in nodes {
        let normalized = PathBuf::from(&node.workspace.path);
        if seen.insert(normalized.clone()) {
            roots.push(normalized.to_string_lossy().to_string());
        }
    }

    roots.sort();
    roots
}

fn resolve_default_workspace_root(online_workspace_roots: &[String]) -> Option<String> {
    if online_workspace_roots.len() == 1 {
        online_workspace_roots.first().cloned()
    } else {
        None
    }
}

fn build_workspace_roots_context(
    online_workspace_roots: &[String],
    default_workspace_root: Option<&str>,
) -> String {
    if online_workspace_roots.is_empty() {
        return "当前没有在线 Node workspace。".to_string();
    }

    let mut lines = vec!["当前在线 Node workspace 列表：".to_string()];
    lines.extend(
        online_workspace_roots
            .iter()
            .map(|root| format!("- {}", root)),
    );

    if let Some(default_workspace_root) = default_workspace_root {
        lines.push(format!("默认 workspace_path：{}", default_workspace_root));
    } else {
        lines.push("存在多个在线 workspace，execute_command 必须显式填写 workspace_path，且必须精确等于其中一个根路径。".to_string());
    }

    lines.join("\n")
}

async fn persist_session_state(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
    conversation_id: &str,
    sender_user_id: Option<&str>,
    sender_staff_id: Option<&str>,
    task_id: Option<&TaskId>,
    execution_workspace_id: Option<&str>,
    collaboration_workspace_id: Option<&str>,
) {
    let resolved_entry = resolve_agent_entry_for_session(state, session_key, Some(agent_id)).await;
    let Some(scope) = resolved_entry
        .as_ref()
        .and_then(agent_scope_from_entry)
        .or_else(|| agent_scope_for(state.as_ref(), agent_id))
    else {
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
    if let Some(entry) = resolved_entry {
        session_state.metadata.insert(
            "agent_source_layer".to_string(),
            entry.source_layer.to_string(),
        );
        if let Some(scope) = entry.source_scope {
            session_state
                .metadata
                .insert("agent_source_scope".to_string(), scope);
        } else {
            session_state.metadata.remove("agent_source_scope");
        }
    }
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
    if let Some(execution_workspace_id) = execution_workspace_id {
        session_state.metadata.insert(
            "execution_workspace_id".to_string(),
            execution_workspace_id.to_string(),
        );
    }
    let collaboration_workspace_id = collaboration_workspace_id
        .map(str::to_string)
        .unwrap_or_else(|| collaboration_workspace_id_for_session(session_key));

    let namespace = session_namespace_for_session(session_key, Some(&session_state));
    write_namespace_metadata(&mut session_state.metadata, &namespace);
    write_collaboration_workspace_runtime_metadata(
        &mut session_state.metadata,
        &collaboration_workspace_id,
        Some(&namespace),
    );

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

    let Some(scope) = resolve_agent_scope_for_session(state, session_key, Some(agent_id)).await
    else {
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

    let session_key = SessionKey::parse(completed_task.context.session_id.as_str()).ok();
    let Some(scope) = task_context_agent_entry(
        state.as_ref(),
        session_key.as_ref(),
        &completed_task.context,
    )
    .and_then(|entry| agent_scope_from_entry(&entry))
    .or_else(|| {
        completed_task
            .context
            .env
            .get("agent_id")
            .and_then(|agent_id| agent_scope_for(state.as_ref(), agent_id))
    }) else {
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

fn skill_to_summary(entry: LayeredSkillEntry) -> SkillRuntimeSummary {
    let skill = entry.skill;
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
        source_layer: entry.source_layer.to_string(),
        source_scope: entry.source_scope,
    }
}

fn skill_to_detail(entry: LayeredSkillEntry) -> SkillRuntimeDetail {
    let skill = entry.skill;
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
        source_layer: entry.source_layer.to_string(),
        source_scope: entry.source_scope,
    }
}

fn session_namespace_from_session_id(session_id: &str) -> Option<SessionNamespace> {
    SessionKey::parse(session_id)
        .ok()
        .map(|key| session_namespace_from_session_key(&key, None))
}

fn session_state_to_detail(session_state: &SessionState) -> SessionRuntimeDetail {
    let mut metadata = session_state.metadata.clone();
    metadata.insert("session_id".to_string(), session_state.session_id.clone());
    let namespace = session_namespace_from_metadata(None, &metadata)
        .or_else(|| session_namespace_from_session_id(&session_state.session_id));
    let memory_context_chain = namespace
        .as_ref()
        .map(SessionNamespace::memory_context_chain)
        .unwrap_or_default();
    let visibility_chain = namespace
        .as_ref()
        .map(SessionNamespace::visibility_chain)
        .unwrap_or_default();
    let default_agent_id = session_state.metadata.get("current_agent").cloned();
    let session_key = SessionKey::parse(&session_state.session_id).ok();
    let collaboration_workspace = build_collaboration_workspace(
        session_key.as_ref(),
        None,
        namespace.as_ref(),
        &metadata,
        default_agent_id.clone(),
    );

    SessionRuntimeDetail {
        session_id: session_state.session_id.clone(),
        agent_id: default_agent_id,
        conversation_id: session_state.metadata.get("conversation_id").cloned(),
        sender_user_id: session_state.metadata.get("sender_user_id").cloned(),
        sender_staff_id: session_state.metadata.get("sender_staff_id").cloned(),
        last_task_id: session_state.metadata.get("last_task_id").cloned(),
        message_count: session_state.message_count,
        created_at: session_state.created_at.to_rfc3339(),
        last_active: session_state.last_active.to_rfc3339(),
        namespace,
        collaboration_workspace,
        memory_context_chain,
        visibility_chain,
        metadata: session_state.metadata.clone(),
    }
}

async fn execute_local_skill(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    skill_name: &str,
    input: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let session_state = load_session_state_for_session(state, session_key).await;
    let visibility_chain =
        session_namespace_for_session(session_key, session_state.as_ref()).visibility_chain();
    let skill = state
        .agent_runtime
        .skills
        .get_for_scopes(&visibility_chain, skill_name)
        .ok_or_else(|| format!("Skill not found: {}", skill_name))?;
    let output = skill.execute(input).await?;
    if output.trim().is_empty() {
        Ok(format!("技能 {} 执行成功，无输出。", skill_name))
    } else {
        Ok(output)
    }
}

fn agent_session_count_key(
    agent_id: &str,
    source_layer: &str,
    source_scope: Option<&str>,
) -> (String, String, Option<String>) {
    (
        agent_id.to_string(),
        source_layer.to_string(),
        source_scope.map(str::to_string),
    )
}

fn resolve_session_agent_count_key(
    state: &WebState,
    session: &SessionRuntimeDetail,
) -> Option<(String, String, Option<String>)> {
    let agent_id = session.agent_id.as_deref()?;
    let entry = session
        .metadata
        .get("agent_source_layer")
        .and_then(|source_layer| {
            state.agent_runtime.agents.get_entry_by_source(
                agent_id,
                source_layer,
                session
                    .metadata
                    .get("agent_source_scope")
                    .map(String::as_str),
            )
        })
        .or_else(|| {
            if session.visibility_chain.is_empty() {
                state.agent_runtime.agents.get_any_entry(agent_id)
            } else {
                state
                    .agent_runtime
                    .agents
                    .get_for_scopes_entry(&session.visibility_chain, agent_id)
                    .or_else(|| state.agent_runtime.agents.get_any_entry(agent_id))
            }
        })?;

    Some(agent_session_count_key(
        agent_id,
        entry.source_layer,
        entry.source_scope.as_deref(),
    ))
}

async fn collect_runtime_sessions(state: &Arc<WebState>) -> Vec<SessionRuntimeDetail> {
    let mut sessions = HashMap::new();
    let mut scanned_scope_dirs = HashSet::new();

    for entry in state.agent_runtime.agents.list_all_entries() {
        let Some(scope) = entry.agent.scope().cloned() else {
            continue;
        };
        let scope_dir = scope.agents_dir().to_string_lossy().to_string();
        if !scanned_scope_dirs.insert(scope_dir) {
            continue;
        }

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
) -> Result<Vec<SessionMessageRecord>, Box<dyn std::error::Error + Send + Sync>> {
    let context = state
        .agent_runtime
        .memory_store
        .get_context(&CoreSessionId::from_string(session_id.to_string()))
        .await?;
    let history = extract_session_history(&context);
    Ok(parse_session_messages(&history))
}

fn extract_session_history(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let marker = "=== Session History ===\n";
    let Some(start) = trimmed.find(marker) else {
        return String::new();
    };

    let rest = &trimmed[start + marker.len()..];
    let end = rest.find("\n\n===").unwrap_or(rest.len());
    rest[..end].trim().to_string()
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
fn render_versioned_template(template: &'static str, version: &str) -> Html<String> {
    Html(template.replace("{{APP_VERSION}}", version))
}

async fn index_page(State(state): State<Arc<WebState>>) -> Html<String> {
    render_versioned_template(
        include_str!("templates/index.html"),
        state.health_service.version(),
    )
}

/// Dashboard 页面
async fn dashboard_page(State(state): State<Arc<WebState>>) -> Html<String> {
    render_versioned_template(
        include_str!("templates/dashboard.html"),
        state.health_service.version(),
    )
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
                None,
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
            let reply_text = execute_local_skill(state, &session_key, &skill_name, &input).await?;
            persist_direct_reply_memory(state, &session_key, &agent_id, text, &reply_text).await;
            persist_session_state(
                state,
                &session_key,
                &agent_id,
                &inbound.conversation_id,
                inbound.sender_user_id.as_deref(),
                inbound.sender_staff_id.as_deref(),
                None,
                None,
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
        AgentDecision::ExecuteCommand {
            command,
            workspace_path,
        } => {
            let online_nodes = state.hub.get_online_nodes().await;
            let online_workspace_roots = collect_online_workspace_roots(&online_nodes);
            let workspace_hint = workspace_path
                .or_else(|| resolve_default_workspace_root(&online_workspace_roots))
                .ok_or_else(|| {
                    if online_workspace_roots.is_empty() {
                        "No online node available".to_string()
                    } else {
                        format!(
                            "Multiple online workspaces available, workspace_path is required: {}",
                            online_workspace_roots.join(", ")
                        )
                    }
                })?;
            let required_capabilities = match &command {
                Command::Browser(_) => Some(NodeCapabilities {
                    supported_commands: vec![CommandType::Browser],
                    ..NodeCapabilities::default()
                }),
                _ => None,
            };
            let execution_workspace_id = online_nodes
                .iter()
                .filter(|node| node.workspace.path == workspace_hint)
                .find(|node| {
                    required_capabilities
                        .as_ref()
                        .map(|required| node.capabilities.meets(required))
                        .unwrap_or(true)
                })
                .and_then(|node| node.workspace.workspace_id.clone());

            validate_planned_command(&command, &workspace_hint)?;

            let session_state = load_session_state_for_session(state, &session_key).await;
            let collaboration_workspace_id = session_state
                .as_ref()
                .and_then(|session_state| {
                    collaboration_workspace_id_from_metadata(
                        &session_state.metadata,
                        Some(&session_key),
                    )
                })
                .unwrap_or_else(|| collaboration_workspace_id_for_session(&session_key));
            let mut task_context = TaskContext::new(
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
            .with_collaboration_workspace_id(collaboration_workspace_id.clone())
            .with_intent(text.to_string())
            .with_env("agent_id", agent_id.clone())
            .with_env("conversation_id", inbound.conversation_id.clone());
            if let Some(execution_workspace_id) = execution_workspace_id.clone() {
                task_context = task_context.with_execution_workspace_id(execution_workspace_id);
            }

            populate_task_context_runtime_metadata(
                &mut task_context,
                Some(&session_key),
                session_state
                    .as_ref()
                    .map(|session_state| &session_state.metadata),
            );

            if let Some(entry) =
                resolve_agent_entry_for_session(state, &session_key, Some(&agent_id)).await
            {
                task_context =
                    task_context.with_env("agent_source_layer", entry.source_layer.to_string());
                if let Some(source_scope) = entry.source_scope {
                    task_context = task_context.with_env("agent_source_scope", source_scope);
                }
            }

            let task_id = state
                .hub
                .submit_task(
                    command,
                    task_context,
                    uhorse_protocol::Priority::Normal,
                    required_capabilities,
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
                execution_workspace_id.as_deref(),
                Some(collaboration_workspace_id.as_str()),
            )
            .await;

            {
                let mut routes = state.dingtalk_routes.write().await;
                routes.insert(task_id.clone(), route.clone());
            }

            if let Some(channel) = state.dingtalk_channel.as_ref() {
                send_dingtalk_reply(channel, &route, DINGTALK_PROCESSING_ACK_TEXT).await?;
            } else {
                warn!("Skip DingTalk processing ack because channel is unavailable");
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
    let session_state = load_session_state_for_session(state, session_key).await;
    let namespace = session_namespace_for_session(session_key, session_state.as_ref());
    let visibility_chain = namespace.visibility_chain();
    let agent = state
        .agent_runtime
        .agents
        .get_for_scopes(&visibility_chain, agent_id)
        .or_else(|| state.agent_runtime.agents.get(agent_id))
        .ok_or_else(|| format!("Agent not found: {}", agent_id))?;
    let context = collect_agent_planning_context(state, agent_id, session_key).await;
    let online_nodes = state.hub.get_online_nodes().await;
    let online_workspace_roots = collect_online_workspace_roots(&online_nodes);
    let default_workspace_root = resolve_default_workspace_root(&online_workspace_roots);
    let response = llm_client
        .chat_completion(build_agent_decision_messages(
            agent.system_prompt(),
            text,
            agent_id,
            session_key,
            &context,
            &online_workspace_roots,
            default_workspace_root.as_deref(),
            &state
                .agent_runtime
                .skills
                .list_names_for_scopes(&visibility_chain),
        ))
        .await?;

    parse_agent_decision(
        state,
        &response,
        text,
        &online_workspace_roots,
        default_workspace_root.as_deref(),
        agent_id,
        session_key,
    )
    .await
}

async fn parse_agent_decision(
    state: &Arc<WebState>,
    response: &str,
    text: &str,
    online_workspace_roots: &[String],
    default_workspace_root: Option<&str>,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<AgentDecision, Box<dyn std::error::Error + Send + Sync>> {
    let default_workspace_root = default_workspace_root.map(ToString::to_string);

    if let Ok(decision) = serde_json::from_str::<AgentDecision>(response) {
        return match decision {
            AgentDecision::ExecuteCommand {
                command,
                workspace_path,
            } => {
                let workspace_path = workspace_path.or_else(|| default_workspace_root.clone());
                if let Some(workspace_path) = workspace_path.as_deref() {
                    validate_planned_command(&command, workspace_path)?;
                }
                Ok(AgentDecision::ExecuteCommand {
                    command,
                    workspace_path,
                })
            }
            other => Ok(other),
        };
    }

    if let Some(default_workspace_root) = default_workspace_root.as_deref() {
        if let Ok(planned) = parse_planned_command(response, default_workspace_root) {
            return Ok(AgentDecision::ExecuteCommand {
                command: planned.command,
                workspace_path: planned.workspace_path,
            });
        }
    }

    let trimmed = response.trim();
    if !trimmed.is_empty() {
        return Ok(AgentDecision::DirectReply {
            text: trimmed.to_string(),
        });
    }

    let Some(default_workspace_root) = default_workspace_root else {
        return Ok(AgentDecision::DirectReply {
            text: response.trim().to_string(),
        });
    };

    plan_dingtalk_command(
        state,
        text,
        &default_workspace_root,
        &online_workspace_roots,
        agent_id,
        session_key,
    )
    .await
}

async fn plan_dingtalk_command(
    state: &Arc<WebState>,
    text: &str,
    workspace_root: &str,
    online_workspace_roots: &[String],
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
            online_workspace_roots,
            agent_id,
            session_key,
            &injected_context,
        ))
        .await?;

    let planned = parse_planned_command(&response, workspace_root)?;
    Ok(AgentDecision::ExecuteCommand {
        command: planned.command,
        workspace_path: planned.workspace_path,
    })
}

fn build_agent_decision_messages(
    agent_system_prompt: &str,
    text: &str,
    agent_id: &str,
    session_key: &SessionKey,
    injected_context: &str,
    online_workspace_roots: &[String],
    default_workspace_root: Option<&str>,
    skill_names: &[String],
) -> Vec<ChatMessage> {
    let workspace_context =
        build_workspace_roots_context(online_workspace_roots, default_workspace_root);
    let mut messages = vec![
        ChatMessage::system(agent_system_prompt.to_string()),
        ChatMessage::system(
            format!(
                "你是 uHorse Hub 的 Agent 决策器。你必须根据用户输入与上下文，只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。允许三种结构：1）直接回复：{{\"type\":\"direct_reply\",\"text\":\"...\"}}；2）需要继续规划命令：{{\"type\":\"execute_command\",\"command\": <uhorse_protocol::Command JSON>, \"workspace_path\": \"...\"}}；3）执行 Hub 本地技能：{{\"type\":\"execute_skill\",\"skill_name\":\"...\",\"input\":\"...\"}}。优先 direct_reply；只有确实需要 Node 执行文件、shell 或浏览器操作时才返回 execute_command。只有当请求明确适合本地技能时才返回 execute_skill。可用技能列表：{}。禁止生成 code/database/api 命令。browser 命令只允许访问安全的 http/https 公网页面，不允许 localhost、127.0.0.1、私网 IP、file:// 等本机或内网目标。用户只是要在宿主机打开网页时使用 open_system；只有需要继续读取网页内容、点击或抓取文本时才使用 navigate / wait_for / get_text / close。若返回 execute_command，workspace_path 必须填写目标 Node workspace 根路径；路径必须限制在该 workspace 内，不允许绝对路径越界，不允许使用 ..。若当前存在多个在线 workspace，workspace_path 必须显式填写，并且只能从提供的在线 workspace 列表中选择。下方的 Agent Workspace Context 和 Session Memory Context 仅供决策参考，不等于 Node 实际工作目录。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。",
                if skill_names.is_empty() {
                    "（无）".to_string()
                } else {
                    skill_names.join(", ")
                }
            )
        ),
        ChatMessage::system(workspace_context),
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
    online_workspace_roots: &[String],
    agent_id: &str,
    session_key: &SessionKey,
    injected_context: &str,
) -> Vec<ChatMessage> {
    let workspace_context =
        build_workspace_roots_context(online_workspace_roots, Some(workspace_root));
    let mut messages = vec![ChatMessage::system(
        "你是 uHorse Hub 的任务规划器。你必须把用户的自然语言请求转换为单个 JSON 对象，且只能输出 JSON，不要输出 Markdown、解释或代码块。JSON 结构必须是 {\"command\": <uhorse_protocol::Command JSON>, \"workspace_path\": \"...\" }。优先生成 file 命令；只有文件命令无法完成时才生成 shell 或 browser 命令。禁止生成 code/database/api/skill 命令。workspace_path 必须填写目标 Node workspace 根路径。路径必须限制在 workspace_path 内，不允许绝对路径越界，不允许使用 ..。若当前存在多个在线 workspace，workspace_path 必须显式填写，并且只能从提供的在线 workspace 列表中选择。下方的 Agent Workspace Context 和 Session Memory Context 仅供参考，不等于 Node 实际工作目录。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。browser 命令只允许访问安全的 http/https 公网页面，不允许 localhost、127.0.0.1、私网 IP、file:// 等本机或内网目标；仅当用户要在宿主机打开网页时使用 open_system；读取网页内容或继续交互时使用 navigate / wait_for / get_text / close。shell 命令只允许只读、安全的本地仓库检查或目录查看。".to_string(),
    )];

    messages.push(ChatMessage::system(format!(
        "{}\n{}",
        workspace_context,
        if injected_context.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\n当前 Agent：{}\n当前 SessionKey：{}\n{}",
                agent_id,
                session_key.as_str(),
                injected_context
            )
        }
    )));

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
) -> Result<PlannedDingTalkCommand, Box<dyn std::error::Error + Send + Sync>> {
    let mut planned: PlannedDingTalkCommand =
        serde_json::from_str(response).map_err(|e| format!("LLM 返回了无效 JSON：{}", e))?;

    if planned.workspace_path.is_none() {
        planned.workspace_path = Some(workspace_root.to_string());
    }

    let effective_workspace = planned.workspace_path.as_deref().unwrap_or(workspace_root);
    validate_planned_command(&planned.command, effective_workspace)?;
    Ok(planned)
}

fn validate_planned_command(
    command: &Command,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        Command::File(file_command) => validate_file_command(file_command, workspace_root),
        Command::Shell(shell_command) => validate_shell_command(shell_command, workspace_root),
        Command::Browser(browser_command) => validate_browser_command(browser_command),
        _ => Err("仅允许规划 FileCommand、ShellCommand 或 BrowserCommand。".into()),
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

fn validate_browser_command(
    command: &uhorse_protocol::BrowserCommand,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        uhorse_protocol::BrowserCommand::OpenSystem { url }
        | uhorse_protocol::BrowserCommand::Navigate { url } => validate_browser_target_url(url),
        uhorse_protocol::BrowserCommand::Screenshot { .. }
        | uhorse_protocol::BrowserCommand::Click { .. }
        | uhorse_protocol::BrowserCommand::Type { .. }
        | uhorse_protocol::BrowserCommand::WaitFor { .. }
        | uhorse_protocol::BrowserCommand::GetText { .. }
        | uhorse_protocol::BrowserCommand::Evaluate { .. }
        | uhorse_protocol::BrowserCommand::Close => Ok(()),
    }
}

fn validate_browser_target_url(url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "浏览器 URL 非法。")?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("浏览器命令只允许 http/https URL。".into()),
    }

    let Some(host) = parsed.host_str() else {
        return Err("浏览器 URL 缺少主机名。".into());
    };

    let host = host.trim().to_ascii_lowercase();
    if matches!(host.as_str(), "localhost" | "localhost.localdomain") {
        return Err("禁止访问 localhost 目标。".into());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_loopback()
                    || ipv4.is_private()
                    || ipv4.is_link_local()
                    || ipv4.is_unspecified()
                    || ipv4.is_broadcast()
                    || ipv4.is_documentation()
                {
                    return Err("禁止访问本机或私网 IPv4 目标。".into());
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback()
                    || ipv6.is_unique_local()
                    || ipv6.is_unicast_link_local()
                    || ipv6.is_unspecified()
                {
                    return Err("禁止访问本机或私网 IPv6 目标。".into());
                }
            }
        }
    }

    Ok(())
}

fn ensure_workspace_path(
    value: &str,
    workspace_root: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = PathBuf::from(value);

    if path.is_absolute() {
        if !workspace_matches_hint(value, workspace_root) {
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

fn resolve_dingtalk_reply_target(route: &DingTalkReplyRoute) -> Option<DingTalkReplyTarget> {
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
        return route
            .session_webhook
            .clone()
            .map(|webhook| DingTalkReplyTarget::SessionWebhook {
                webhook,
                at_user_ids,
            });
    }

    let is_group = matches!(route.conversation_type.as_deref(), Some("2"));
    if is_group {
        return Some(DingTalkReplyTarget::GroupConversation {
            conversation_id: route.conversation_id.clone(),
        });
    }

    route
        .sender_user_id
        .clone()
        .map(|user_id| DingTalkReplyTarget::DirectUser { user_id })
}

async fn send_dingtalk_reply(
    channel: &DingTalkChannel,
    route: &DingTalkReplyRoute,
    reply_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match resolve_dingtalk_reply_target(route) {
        Some(DingTalkReplyTarget::SessionWebhook {
            webhook,
            at_user_ids,
        }) => {
            channel
                .reply_via_session_webhook(&webhook, reply_text, &at_user_ids)
                .await?;
        }
        Some(DingTalkReplyTarget::GroupConversation { conversation_id }) => {
            channel
                .send_group_message(
                    &conversation_id,
                    &MessageContent::Text(reply_text.to_string()),
                )
                .await?;
        }
        Some(DingTalkReplyTarget::DirectUser { user_id }) => {
            channel
                .send_message(&user_id, &MessageContent::Text(reply_text.to_string()))
                .await?;
        }
        None => {
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

    if let Some(reply) = result_summary_override(&completed_task.result) {
        return reply;
    }

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

fn result_summary_override(result: &uhorse_protocol::CommandResult) -> Option<String> {
    if !result.success {
        return None;
    }

    match &result.output {
        CommandOutput::Json { content } => format_file_operation_result(content),
        _ => None,
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

fn format_file_operation_result(content: &serde_json::Value) -> Option<String> {
    if content.get("kind")?.as_str()? != "file_operation" {
        return None;
    }

    let action = content.get("action")?.as_str()?;
    let path = content
        .get("path")
        .and_then(|value| value.as_str())
        .or_else(|| {
            content
                .get("destination_path")
                .and_then(|value| value.as_str())
        });

    match action {
        "write" => path.map(|path| format!("已保存成功：{}", path)),
        "append" => path.map(|path| format!("已追加成功：{}", path)),
        "copy" => match (
            content.get("source_path").and_then(|value| value.as_str()),
            content
                .get("destination_path")
                .and_then(|value| value.as_str()),
        ) {
            (Some(source), Some(destination)) => {
                Some(format!("已复制成功：{}\n到：{}", source, destination))
            }
            _ => path.map(|path| format!("已复制成功：{}", path)),
        },
        "move" => match (
            content.get("source_path").and_then(|value| value.as_str()),
            content
                .get("destination_path")
                .and_then(|value| value.as_str()),
        ) {
            (Some(source), Some(destination)) => {
                Some(format!("已移动成功：{}\n到：{}", source, destination))
            }
            _ => path.map(|path| format!("已移动成功：{}", path)),
        },
        "create_dir" => path.map(|path| format!("已创建目录：{}", path)),
        _ => None,
    }
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
            format_file_operation_result(content).unwrap_or_else(|| {
                serde_json::to_string_pretty(content).unwrap_or_else(|_| content.to_string())
            })
        }
        CommandOutput::Browser { result } => match result {
            BrowserResult::OpenSystem { url } => format!("已在系统浏览器打开：{}", url),
            BrowserResult::Navigate { final_url, title } => match title {
                Some(title) if !title.trim().is_empty() => {
                    format!("浏览器会话已导航到：{}\n标题：{}", final_url, title)
                }
                _ => format!("浏览器会话已导航到：{}", final_url),
            },
            BrowserResult::GetText { text } => {
                if text.trim().is_empty() {
                    "页面已读取，但未提取到文本。".to_string()
                } else {
                    text.clone()
                }
            }
            BrowserResult::Evaluate { value } => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
            BrowserResult::Ok => "执行成功。".to_string(),
            BrowserResult::Error { message } => format!("执行失败：{}", message),
            BrowserResult::Screenshot { format, .. } => {
                format!("截图成功，格式：{}。", format)
            }
        },
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
    let session_counts: HashMap<(String, String, Option<String>), usize> =
        sessions.into_iter().fold(HashMap::new(), |mut acc, item| {
            if let Some(key) = resolve_session_agent_count_key(state.as_ref(), &item) {
                *acc.entry(key).or_insert(0) += 1;
            }
            acc
        });

    let mut agents: Vec<_> = state
        .agent_runtime
        .agents
        .list_all_entries()
        .into_iter()
        .map(|entry| {
            let agent = entry.agent;
            let key = agent_session_count_key(
                agent.agent_id(),
                entry.source_layer,
                entry.source_scope.as_deref(),
            );
            AgentRuntimeSummary {
                agent_id: agent.agent_id().to_string(),
                name: agent.name().to_string(),
                description: agent.description().to_string(),
                workspace_dir: agent.workspace_dir().display().to_string(),
                is_default: agent
                    .scope()
                    .map(|scope| scope.config().is_default)
                    .unwrap_or(false),
                skill_names: state.agent_runtime.skills.list_all_names(),
                active_session_count: session_counts.get(&key).copied().unwrap_or(0),
                source_layer: entry.source_layer.to_string(),
                source_scope: entry.source_scope,
            }
        })
        .collect();
    agents.sort_by(|left, right| {
        left.agent_id
            .cmp(&right.agent_id)
            .then_with(|| left.source_layer.cmp(&right.source_layer))
            .then_with(|| {
                left.source_scope
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.source_scope.as_deref().unwrap_or(""))
            })
    });

    Json(ApiResponse::success(agents))
}

async fn get_runtime_agent(
    State(state): State<Arc<WebState>>,
    Path(agent_id): Path<String>,
    Query(query): Query<AgentRuntimeQuery>,
) -> (StatusCode, Json<ApiResponse<AgentRuntimeDetail>>) {
    let sessions = collect_runtime_sessions(&state).await;
    let entry = match query.source_layer.as_deref() {
        Some(source_layer) => state.agent_runtime.agents.get_entry_by_source(
            &agent_id,
            source_layer,
            query.source_scope.as_deref(),
        ),
        None => state.agent_runtime.agents.get_any_entry(&agent_id),
    };

    match entry {
        Some(entry) => {
            let active_session_count = sessions
                .iter()
                .filter_map(|session| resolve_session_agent_count_key(state.as_ref(), session))
                .filter(|key| {
                    *key == agent_session_count_key(
                        &agent_id,
                        entry.source_layer,
                        entry.source_scope.as_deref(),
                    )
                })
                .count();
            let agent = entry.agent;
            (
                StatusCode::OK,
                Json(ApiResponse::success(AgentRuntimeDetail {
                    agent_id: agent.agent_id().to_string(),
                    name: agent.name().to_string(),
                    description: agent.description().to_string(),
                    workspace_dir: agent.workspace_dir().display().to_string(),
                    system_prompt: agent.system_prompt().to_string(),
                    is_default: agent
                        .scope()
                        .map(|scope| scope.config().is_default)
                        .unwrap_or(false),
                    skill_names: state.agent_runtime.skills.list_all_names(),
                    active_session_count,
                    source_layer: entry.source_layer.to_string(),
                    source_scope: entry.source_scope,
                })),
            )
        }
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
        .list_all_entries()
        .into_iter()
        .map(skill_to_summary)
        .collect();
    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.source_layer.cmp(&right.source_layer))
            .then_with(|| {
                left.source_scope
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.source_scope.as_deref().unwrap_or(""))
            })
    });
    Json(ApiResponse::success(skills))
}

async fn get_runtime_skill(
    State(state): State<Arc<WebState>>,
    Path(skill_name): Path<String>,
    Query(query): Query<SkillRuntimeQuery>,
) -> (StatusCode, Json<ApiResponse<SkillRuntimeDetail>>) {
    let entry = match query.source_layer.as_deref() {
        Some(source_layer) => state.agent_runtime.skills.get_entry_by_source(
            &skill_name,
            source_layer,
            query.source_scope.as_deref(),
        ),
        None => state.agent_runtime.skills.get_any_entry(&skill_name),
    };

    match entry {
        Some(entry) => (
            StatusCode::OK,
            Json(ApiResponse::success(skill_to_detail(entry))),
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
            namespace: session.namespace,
            collaboration_workspace: session.collaboration_workspace,
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
    let Some(_session) = collect_runtime_sessions(&state)
        .await
        .into_iter()
        .find(|item| item.session_id == session_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Session not found")),
        );
    };

    match read_session_messages(&state, &session_id).await {
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
    /// 执行工作空间 ID
    pub execution_workspace_id: Option<String>,
    /// 逻辑协作工作空间
    pub collaboration_workspace: Option<CollaborationWorkspace>,
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
    execution_workspace_id: Option<String>,
    #[serde(default)]
    collaboration_workspace_id: Option<String>,
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
fn task_info_from_status_and_context(
    status: crate::task_scheduler::TaskStatusInfo,
    context: Option<TaskContext>,
) -> TaskInfo {
    let mut metadata = context
        .as_ref()
        .map(|context| context.env.clone())
        .unwrap_or_default();
    if let Some(session_id) = context
        .as_ref()
        .map(|context| context.session_id.as_str().to_string())
    {
        metadata.insert("session_id".to_string(), session_id);
    }
    if let Some(execution_workspace_id) = context
        .as_ref()
        .and_then(|context| context.execution_workspace_id.clone())
    {
        metadata.insert("execution_workspace_id".to_string(), execution_workspace_id);
    }
    let session_key = context
        .as_ref()
        .and_then(|context| SessionKey::parse(context.session_id.as_str()).ok());
    let namespace = context
        .as_ref()
        .and_then(|context| task_context_namespace(session_key.as_ref(), context));
    let collaboration_workspace = build_collaboration_workspace(
        session_key.as_ref(),
        context
            .as_ref()
            .and_then(|context| context.collaboration_workspace_id.clone()),
        namespace.as_ref(),
        &metadata,
        context
            .as_ref()
            .and_then(|context| context.env.get("agent_id").cloned()),
    );

    TaskInfo {
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
        execution_workspace_id: context.and_then(|context| context.execution_workspace_id),
        collaboration_workspace,
        started_at: status.started_at.map(|t| t.to_rfc3339()),
    }
}

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
    if let Some(execution_workspace_id) = payload.execution_workspace_id {
        context = context.with_execution_workspace_id(execution_workspace_id);
    }
    if let Some(collaboration_workspace_id) = payload.collaboration_workspace_id {
        context = context.with_collaboration_workspace_id(collaboration_workspace_id);
    }
    if let Some(intent) = payload.intent {
        context = context.with_intent(intent);
    }
    for (key, value) in payload.env {
        context = context.with_env(key, value);
    }
    let session_key = SessionKey::parse(context.session_id.as_str()).ok();
    populate_task_context_runtime_metadata(&mut context, session_key.as_ref(), None);

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
    let task_id = uhorse_protocol::TaskId::from_string(&task_id);
    match state.hub.get_task_status(&task_id).await {
        Some(status) => {
            let context = state.hub.get_task_context(&task_id).await;
            let info = task_info_from_status_and_context(status, context);
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
        Action, BrowserCommand, BrowserResult, Command, CommandOutput, CommandResult, CommandType,
        ExecutionError, FileCommand, HubToNode, NodeCapabilities, NodeToHub, Priority,
        ResourcePattern as ProtocolResourcePattern, ShellCommand, TaskStatus, WorkspaceInfo,
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
                workspace_id: None,
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
    async fn test_index_page_renders_runtime_version() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let app = create_router(WebState::new(Arc::new(hub), None, None));
        let (status, body, _) = get_text(app, "/").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
        assert!(!body.contains("Version {{APP_VERSION}}"));
    }

    #[tokio::test]
    async fn test_dashboard_page_renders_runtime_version() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let app = create_router(WebState::new(Arc::new(hub), None, None));
        let (status, body, _) = get_text(app, "/dashboard").await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
        assert!(!body.contains("v{{APP_VERSION}}"));
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

        let planned = parse_planned_command(&response, workspace).unwrap();
        assert_eq!(planned.workspace_path.as_deref(), Some(workspace));
        match planned.command {
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

    #[test]
    fn test_result_summary_override_returns_file_operation_reply() {
        let result = CommandResult::success(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "write",
            "path": "/tmp/report.md",
            "bytes_written": 12,
        })));

        assert_eq!(
            result_summary_override(&result),
            Some("已保存成功：/tmp/report.md".to_string())
        );
    }

    #[tokio::test]
    async fn test_build_task_result_reply_text_prefers_file_operation_override() {
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, mut task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: "这条结果不应该被使用".to_string(),
            })),
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-file-reply");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id.clone(),
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
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

        let task_id = hub
            .submit_task(
                Command::File(FileCommand::Write {
                    path: format!("{}/report.md", workspace_root),
                    content: "hello world".to_string(),
                    overwrite: true,
                }),
                TaskContext::new(
                    UserId::from_string("user-1"),
                    uhorse_protocol::SessionId::from_string("session-1"),
                    "dingtalk",
                )
                .with_intent("保存报告"),
                Priority::Normal,
                None,
                vec![],
                Some(workspace_root.clone()),
            )
            .await
            .unwrap();

        let assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        match assignment {
            HubToNode::TaskAssignment {
                task_id: assigned_task_id,
                command,
                ..
            } => {
                assert_eq!(assigned_task_id, task_id);
                match command {
                    Command::File(FileCommand::Write { path, .. }) => {
                        assert_eq!(path, format!("{}/report.md", workspace_root));
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
            }
            other => panic!("unexpected message: {:?}", other),
        }

        let result = CommandResult::success(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "write",
            "path": format!("{}/report.md", workspace_root),
            "bytes_written": 11,
        })));
        hub.handle_node_message(
            &node_id,
            NodeToHub::TaskResult {
                message_id: MessageId::new(),
                task_id: task_id.clone(),
                result,
                metrics: uhorse_protocol::ExecutionMetrics {
                    duration_ms: 1,
                    cpu_time_ms: 0,
                    peak_memory_mb: 0,
                    bytes_read: 0,
                    bytes_written: 11,
                    network_requests: 0,
                },
            },
        )
        .await
        .unwrap();

        let task_result =
            tokio::time::timeout(std::time::Duration::from_secs(1), task_result_rx.recv())
                .await
                .unwrap()
                .unwrap();
        let reply = build_task_result_reply_text(&state, &task_result).await;

        assert_eq!(reply, format!("已保存成功：{}/report.md", workspace_root));
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
            &[workspace_root.clone()],
            "main",
            &session_key,
        )
        .await
        .unwrap();

        match decision {
            AgentDecision::ExecuteCommand {
                command: Command::File(FileCommand::Exists { path }),
                workspace_path,
            } => {
                assert_eq!(path, format!("{}/Cargo.toml", workspace_root));
                assert_eq!(workspace_path.as_deref(), Some(workspace_root.as_str()));
            }
            other => panic!("unexpected decision: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_plan_dingtalk_command_allows_browser_output() {
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"command":{"type":"browser","action":"open_system","url":"https://example.com/article"}}"#.to_string(),
            })),
        ));

        let session_key = SessionKey::new("dingtalk", "user-1");
        let decision = plan_dingtalk_command(
            &state,
            "打开文章页面",
            &workspace_root,
            &[workspace_root.clone()],
            "main",
            &session_key,
        )
        .await
        .unwrap();

        match decision {
            AgentDecision::ExecuteCommand {
                command: Command::Browser(uhorse_protocol::BrowserCommand::OpenSystem { url }),
                workspace_path,
            } => {
                assert_eq!(url, "https://example.com/article");
                assert_eq!(workspace_path.as_deref(), Some(workspace_root.as_str()));
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
            None,
            None,
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
                .get("agent_source_layer")
                .map(String::as_str),
            Some("global")
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
                .get("collaboration_workspace_id")
                .map(String::as_str),
            Some("collab:session:dingtalk:user-1:corp-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("collaboration_scope_owner")
                .map(String::as_str),
            Some("session:dingtalk:user-1:corp-1")
        );
        assert_eq!(
            session_state
                .metadata
                .get("collaboration_materialization")
                .map(String::as_str),
            Some("none")
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

    #[test]
    fn test_session_namespace_from_metadata_restores_enterprise_chain() {
        let session_key = SessionKey::with_team("dingtalk", "user-1", "corp-1");
        let metadata = HashMap::from([
            ("namespace_global".to_string(), "global".to_string()),
            (
                "namespace_tenant".to_string(),
                "tenant:dingtalk:corp-1".to_string(),
            ),
            (
                "namespace_enterprise".to_string(),
                "enterprise:org-1".to_string(),
            ),
            (
                "namespace_department".to_string(),
                "department:org-1:sales".to_string(),
            ),
            (
                "namespace_roles".to_string(),
                serde_json::to_string(&vec!["role:org-1:manager"]).unwrap(),
            ),
            (
                "namespace_user".to_string(),
                "user:dingtalk:user-1".to_string(),
            ),
            (
                "namespace_session".to_string(),
                "session:dingtalk:user-1:corp-1".to_string(),
            ),
        ]);

        let namespace = session_namespace_from_metadata(Some(&session_key), &metadata).unwrap();
        assert_eq!(namespace.enterprise.as_deref(), Some("enterprise:org-1"));
        assert_eq!(
            namespace.department.as_deref(),
            Some("department:org-1:sales")
        );
        assert_eq!(namespace.roles, vec!["role:org-1:manager".to_string()]);
        assert_eq!(
            namespace.visibility_chain(),
            vec![
                "user:dingtalk:user-1".to_string(),
                "role:org-1:manager".to_string(),
                "department:org-1:sales".to_string(),
                "enterprise:org-1".to_string(),
                "tenant:dingtalk:corp-1".to_string(),
                "global".to_string(),
            ]
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
        assert!(context.contains("Collaboration Workspace Context"));
        assert!(context.contains("collab:session:dingtalk:user-ctx"));
        assert!(context.contains("这是 Hub 侧逻辑协作空间，不是 Node 实际执行目录"));
        assert!(context.contains("MEMORY.md"));
        assert!(context.contains("global facts"));
        assert!(context.contains("Session Memory Context"));
        assert!(context.contains("**User:** hello"));
        assert!(context.contains("**Assistant:** world"));
    }

    #[test]
    fn test_render_collaboration_workspace_context_formats_fields() {
        let context = render_collaboration_workspace_context(&CollaborationWorkspace {
            collaboration_workspace_id: "collab:department:org-1:sales".to_string(),
            scope_owner: "department:org-1:sales".to_string(),
            members: vec!["user-1".to_string(), "staff-1".to_string()],
            default_agent_id: Some("helper".to_string()),
            visible_scope_chain: vec![
                "user:dingtalk:user-1".to_string(),
                "department:org-1:sales".to_string(),
                "enterprise:org-1".to_string(),
                "global".to_string(),
            ],
            bound_execution_workspace_id: Some("exec:node-1:workspace".to_string()),
            materialization: "none".to_string(),
        });

        assert!(context.contains("Collaboration Workspace Context"));
        assert!(context.contains("collaboration_workspace_id: collab:department:org-1:sales"));
        assert!(context.contains("scope_owner: department:org-1:sales"));
        assert!(context.contains("default_agent_id: helper"));
        assert!(context.contains("bound_execution_workspace_id: exec:node-1:workspace"));
        assert!(context.contains("materialization: none"));
        assert!(context.contains("members: user-1, staff-1"));
        assert!(context.contains(
            "visible_scope_chain: user:dingtalk:user-1 -> department:org-1:sales -> enterprise:org-1 -> global"
        ));
        assert!(context.contains("这是 Hub 侧逻辑协作空间，不是 Node 实际执行目录"));
    }

    #[test]
    fn test_session_state_to_detail_includes_namespace_chains() {
        let mut session_state = SessionState::new("dingtalk:user-1:corp-1".to_string());
        session_state
            .metadata
            .insert("current_agent".to_string(), "main".to_string());
        session_state.metadata.insert(
            "namespace_enterprise".to_string(),
            "enterprise:org-1".to_string(),
        );
        session_state.metadata.insert(
            "namespace_department".to_string(),
            "department:org-1:sales".to_string(),
        );
        session_state.metadata.insert(
            "namespace_roles".to_string(),
            serde_json::to_string(&vec!["role:org-1:manager"]).unwrap(),
        );

        let detail = session_state_to_detail(&session_state);

        assert_eq!(
            detail.namespace.as_ref().map(|ns| ns.global.as_str()),
            Some("global")
        );
        assert_eq!(
            detail
                .namespace
                .as_ref()
                .and_then(|ns| ns.tenant.as_deref()),
            Some("tenant:dingtalk:corp-1")
        );
        assert_eq!(
            detail
                .namespace
                .as_ref()
                .and_then(|ns| ns.enterprise.as_deref()),
            Some("enterprise:org-1")
        );
        assert_eq!(
            detail.memory_context_chain,
            vec![
                "global".to_string(),
                "tenant:dingtalk:corp-1".to_string(),
                "enterprise:org-1".to_string(),
                "department:org-1:sales".to_string(),
                "role:org-1:manager".to_string(),
                "user:dingtalk:user-1".to_string(),
                "session:dingtalk:user-1:corp-1".to_string()
            ]
        );
        assert_eq!(
            detail.visibility_chain,
            vec![
                "user:dingtalk:user-1".to_string(),
                "role:org-1:manager".to_string(),
                "department:org-1:sales".to_string(),
                "enterprise:org-1".to_string(),
                "tenant:dingtalk:corp-1".to_string(),
                "global".to_string()
            ]
        );
        assert_eq!(
            detail
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.collaboration_workspace_id.as_str()),
            Some("collab:session:dingtalk:user-1:corp-1")
        );
        assert_eq!(
            detail
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.scope_owner.as_str()),
            Some("session:dingtalk:user-1:corp-1")
        );
    }

    #[test]
    fn test_session_state_to_detail_restores_custom_collaboration_workspace_metadata() {
        let mut session_state = SessionState::new("dingtalk:user-1:corp-1".to_string());
        session_state
            .metadata
            .insert("current_agent".to_string(), "helper".to_string());
        session_state.metadata.insert(
            "execution_workspace_id".to_string(),
            "exec:node-1:workspace".to_string(),
        );
        session_state.metadata.insert(
            "collaboration_workspace_id".to_string(),
            "collab:department:org-1:sales".to_string(),
        );
        session_state.metadata.insert(
            "collaboration_scope_owner".to_string(),
            "department:org-1:sales".to_string(),
        );
        session_state.metadata.insert(
            "collaboration_materialization".to_string(),
            "none".to_string(),
        );
        session_state
            .metadata
            .insert("sender_user_id".to_string(), "user-1".to_string());
        session_state.metadata.insert(
            "namespace_department".to_string(),
            "department:org-1:sales".to_string(),
        );

        let detail = session_state_to_detail(&session_state);
        let workspace = detail
            .collaboration_workspace
            .as_ref()
            .expect("collaboration workspace should be restored");

        assert_eq!(
            workspace.collaboration_workspace_id,
            "collab:department:org-1:sales"
        );
        assert_eq!(workspace.scope_owner, "department:org-1:sales");
        assert_eq!(workspace.default_agent_id.as_deref(), Some("helper"));
        assert_eq!(
            workspace.bound_execution_workspace_id.as_deref(),
            Some("exec:node-1:workspace")
        );
        assert_eq!(workspace.materialization, "none");
        assert!(workspace
            .visible_scope_chain
            .contains(&"department:org-1:sales".to_string()));
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
    async fn test_collect_runtime_sessions_reads_user_catalog_scope() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let user_root = runtime_root.join("users").join("user:dingtalk:user-scope");
        tokio::fs::create_dir_all(user_root.join("workspace-helper"))
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::new("dingtalk", "user-scope");

        persist_session_state(
            &state,
            &session_key,
            "helper",
            "conv-user-scope",
            Some("user-scope"),
            None,
            None,
            None,
            None,
        )
        .await;

        let sessions = collect_runtime_sessions(&state).await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_key.as_str());
        assert_eq!(sessions[0].agent_id.as_deref(), Some("helper"));
    }

    #[tokio::test]
    async fn test_list_runtime_agents_includes_source_metadata_for_user_catalog() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let user_scope = "user:dingtalk:user-metadata";
        let user_root = runtime_root.join("users").join(user_scope);
        tokio::fs::create_dir_all(user_root.join("workspace-helper"))
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-metadata");

        persist_session_state(
            &state,
            &session_key,
            "helper",
            "conv-user-metadata",
            Some("user-metadata"),
            None,
            None,
            None,
            None,
        )
        .await;

        let Json(response) = list_runtime_agents(State(state)).await;
        let agents = response.data.unwrap();
        let helper = agents
            .into_iter()
            .find(|agent| {
                agent.agent_id == "helper"
                    && agent.source_layer == "user"
                    && agent.source_scope.as_deref() == Some(user_scope)
            })
            .expect("user helper agent");
        assert_eq!(helper.active_session_count, 1);
    }

    #[tokio::test]
    async fn test_get_runtime_agent_returns_requested_source_entry() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let tenant_root = runtime_root.join("tenants").join(tenant_scope);
        let user_root = runtime_root.join("users").join("user:dingtalk:user-shared");
        tokio::fs::create_dir_all(tenant_root.join("workspace-helper"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(user_root.join("workspace-helper"))
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let app = create_router(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime,
        ));

        let (status, body) = get_json(
            app,
            "/api/v1/agents/helper?source_layer=tenant&source_scope=tenant%3Adingtalk%3Acorp-shared",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["agent_id"], json!("helper"));
        assert_eq!(body["data"]["source_layer"], json!("tenant"));
        assert_eq!(
            body["data"]["source_scope"],
            json!("tenant:dingtalk:corp-shared")
        );
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

    #[test]
    fn test_task_context_agent_entry_prefers_source_metadata() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let user_scope = "user:dingtalk:user-shared";
        std::fs::create_dir_all(
            runtime_root
                .join("tenants")
                .join(tenant_scope)
                .join("workspace-helper"),
        )
        .unwrap();
        std::fs::create_dir_all(
            runtime_root
                .join("users")
                .join(user_scope)
                .join("workspace-helper"),
        )
        .unwrap();

        let runtime = tokio_test::block_on(init_default_agent_runtime(runtime_root)).unwrap();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new_with_runtime(Arc::new(hub), None, None, Arc::new(runtime));
        let session_key = SessionKey::with_team("dingtalk", "user-shared", "corp-shared");
        let context = TaskContext::new(
            UserId::from_string("user-shared"),
            uhorse_protocol::SessionId::from_string(session_key.as_str()),
            "dingtalk",
        )
        .with_env("agent_id", "helper")
        .with_env("agent_source_layer", "tenant")
        .with_env("agent_source_scope", tenant_scope);

        let entry = task_context_agent_entry(&state, Some(&session_key), &context).unwrap();
        assert_eq!(entry.source_layer, "tenant");
        assert_eq!(entry.source_scope.as_deref(), Some(tenant_scope));
    }

    #[test]
    fn test_task_context_agent_entry_uses_namespace_metadata_chain() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let enterprise_scope = "enterprise:org-1";
        let role_scope = "role:org-1:manager";
        std::fs::create_dir_all(
            runtime_root
                .join("enterprises")
                .join(enterprise_scope)
                .join("workspace-helper"),
        )
        .unwrap();
        std::fs::create_dir_all(
            runtime_root
                .join("roles")
                .join(role_scope)
                .join("workspace-helper"),
        )
        .unwrap();

        let runtime = tokio_test::block_on(init_default_agent_runtime(runtime_root)).unwrap();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new_with_runtime(Arc::new(hub), None, None, Arc::new(runtime));
        let session_key = SessionKey::with_team("dingtalk", "user-shared", "corp-shared");
        let context = TaskContext::new(
            UserId::from_string("user-shared"),
            uhorse_protocol::SessionId::from_string(session_key.as_str()),
            "dingtalk",
        )
        .with_env("agent_id", "helper")
        .with_env("namespace_enterprise", enterprise_scope)
        .with_env(
            "namespace_roles",
            serde_json::to_string(&vec![role_scope]).unwrap(),
        );

        let entry = task_context_agent_entry(&state, Some(&session_key), &context).unwrap();
        assert_eq!(entry.source_layer, "role");
        assert_eq!(entry.source_scope.as_deref(), Some(role_scope));
    }

    #[tokio::test]
    async fn test_collect_agent_planning_context_uses_session_bound_scope() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let user_scope = "user:dingtalk:user-shared";
        let tenant_root = runtime_root
            .join("tenants")
            .join(tenant_scope)
            .join("workspace-helper");
        let user_root = runtime_root
            .join("users")
            .join(user_scope)
            .join("workspace-helper");
        tokio::fs::create_dir_all(&tenant_root).await.unwrap();
        tokio::fs::create_dir_all(&user_root).await.unwrap();
        tokio::fs::write(tenant_root.join("AGENTS.md"), "tenant instructions")
            .await
            .unwrap();
        tokio::fs::write(user_root.join("AGENTS.md"), "user instructions")
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: "直接回复用户".to_string(),
            })),
            runtime,
        ));
        let session_key = SessionKey::with_team("dingtalk", "user-shared", "corp-shared");

        persist_session_state(
            &state,
            &session_key,
            "helper",
            "conv-shared",
            Some("user-shared"),
            None,
            None,
            None,
            None,
        )
        .await;

        let context = collect_agent_planning_context(&state, "helper", &session_key).await;
        assert!(context.contains("AGENTS.md"));
        assert!(context.contains("user instructions"));
        assert!(!context.contains("tenant instructions"));
    }

    #[tokio::test]
    async fn test_collect_agent_planning_context_uses_metadata_namespace_memory_chain() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let enterprise_scope = "enterprise:org-1";
        let role_scope = "role:org-1:manager";
        let enterprise_memory_root = runtime_root
            .join("workspace")
            .join("enterprises")
            .join(enterprise_scope);
        let role_root = runtime_root
            .join("roles")
            .join(role_scope)
            .join("workspace-helper");
        tokio::fs::create_dir_all(&enterprise_memory_root)
            .await
            .unwrap();
        tokio::fs::create_dir_all(&role_root).await.unwrap();
        tokio::fs::write(
            enterprise_memory_root.join("MEMORY.md"),
            "enterprise memory",
        )
        .await
        .unwrap();
        tokio::fs::write(role_root.join("AGENTS.md"), "role instructions")
            .await
            .unwrap();

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: "直接回复用户".to_string(),
            })),
            runtime,
        ));
        let session_key = SessionKey::with_team("dingtalk", "user-shared", "corp-shared");
        let scope = state
            .agent_runtime
            .agents
            .get_entry_by_source("helper", "role", Some(role_scope))
            .and_then(|entry| agent_scope_from_entry(&entry))
            .unwrap();
        let mut session_state = SessionState::new(session_key.as_str());
        session_state
            .metadata
            .insert("current_agent".to_string(), "helper".to_string());
        session_state
            .metadata
            .insert("agent_source_layer".to_string(), "role".to_string());
        session_state
            .metadata
            .insert("agent_source_scope".to_string(), role_scope.to_string());
        session_state.metadata.insert(
            "namespace_enterprise".to_string(),
            enterprise_scope.to_string(),
        );
        session_state.metadata.insert(
            "namespace_roles".to_string(),
            serde_json::to_string(&vec![role_scope]).unwrap(),
        );
        scope
            .save_session_state(&session_key.as_str(), &session_state)
            .await
            .unwrap();

        let context = collect_agent_planning_context(&state, "helper", &session_key).await;
        assert!(context.contains("enterprise memory"));
        assert!(context.contains("role instructions"));
        assert!(
            context.contains("enterprise: org-1")
                || context.contains("enterprise: enterprise:org-1")
        );
        assert!(context.contains(role_scope));
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
                    r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}},"workspace_path":"{}"}}"#,
                    workspace_root, workspace_root
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
                workspace_id: None,
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
        assert_eq!(
            context.env.get("namespace_global").map(String::as_str),
            Some("global")
        );
        assert_eq!(
            context.env.get("namespace_tenant").map(String::as_str),
            Some("tenant:dingtalk:corp-1")
        );
        assert_eq!(
            context.env.get("namespace_user").map(String::as_str),
            Some("user:dingtalk:actual-user")
        );
        assert_eq!(
            context.env.get("namespace_session").map(String::as_str),
            Some("session:dingtalk:actual-user:corp-1")
        );
        assert_eq!(
            context.collaboration_workspace_id.as_deref(),
            Some("collab:session:dingtalk:actual-user:corp-1")
        );
        assert_eq!(
            context
                .env
                .get("collaboration_workspace_id")
                .map(String::as_str),
            Some("collab:session:dingtalk:actual-user:corp-1")
        );
        assert_eq!(
            context
                .env
                .get("collaboration_scope_owner")
                .map(String::as_str),
            Some("session:dingtalk:actual-user:corp-1")
        );
        assert_eq!(
            context
                .env
                .get("collaboration_materialization")
                .map(String::as_str),
            Some("none")
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
        assert_eq!(
            session_state
                .metadata
                .get("collaboration_workspace_id")
                .map(String::as_str),
            Some("collab:session:dingtalk:actual-user:corp-1")
        );
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_propagates_existing_namespace_metadata_to_task_context() {
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
                    r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}},"workspace_path":"{}"}}"#,
                    workspace_root, workspace_root
                ),
            })),
            runtime.clone(),
        ));

        let node_id = uhorse_protocol::NodeId::from_string("node-submit-metadata");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
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

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let serialized_roles = serde_json::to_string(&vec!["role:org-1:manager"]).unwrap();
        let mut session_state = SessionState::new(session_key.as_str());
        session_state
            .metadata
            .insert("current_agent".to_string(), "main".to_string());
        let namespace = session_key.namespace_with_access_context(Some(&AccessContext {
            tenant: None,
            enterprise: Some("enterprise:org-1".to_string()),
            department: Some("department:org-1:sales".to_string()),
            roles: vec!["role:org-1:manager".to_string()],
        }));
        write_namespace_metadata(&mut session_state.metadata, &namespace);
        runtime
            .agent_manager
            .get_scope("main")
            .unwrap()
            .save_session_state(&session_key.as_str(), &session_state)
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
            conversation_id: "conv-submit-metadata".to_string(),
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
        let context = match assignment {
            HubToNode::TaskAssignment { context, .. } => context,
            other => panic!("unexpected message: {:?}", other),
        };

        assert_eq!(
            context.env.get("namespace_enterprise").map(String::as_str),
            Some("enterprise:org-1")
        );
        assert_eq!(
            context.env.get("namespace_department").map(String::as_str),
            Some("department:org-1:sales")
        );
        assert_eq!(
            context.env.get("namespace_roles").map(String::as_str),
            Some(serialized_roles.as_str())
        );
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_requires_workspace_path_when_multiple_workspaces_online() {
        let runtime = create_test_runtime().await;
        let workspace_a = tempdir().unwrap();
        let workspace_b = tempdir().unwrap();
        let workspace_a_root = workspace_a.path().to_string_lossy().to_string();
        let workspace_b_root = workspace_b.path().to_string_lossy().to_string();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: format!(
                    r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/Cargo.toml"}}}}"#,
                    workspace_a_root
                ),
            })),
            runtime,
        ));

        let node_a_id = uhorse_protocol::NodeId::from_string("node-a");
        let (node_a_tx, _node_a_rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_a_id.clone(), node_a_tx)
            .await;
        hub.handle_node_connection(
            node_a_id,
            "node-a".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace-a".to_string(),
                path: workspace_a_root.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();

        let node_b_id = uhorse_protocol::NodeId::from_string("node-b");
        let (node_b_tx, _node_b_rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_b_id.clone(), node_b_tx)
            .await;
        hub.handle_node_connection(
            node_b_id,
            "node-b".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace-b".to_string(),
                path: workspace_b_root,
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
            conversation_id: "conv-multi-workspace".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        let error = submit_dingtalk_task(&state, inbound).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("Multiple online workspaces available, workspace_path is required"));
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_dispatches_browser_assignment_only_to_browser_capable_node()
    {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, mut task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: format!(
                    r#"{{"type":"execute_command","command":{{"type":"browser","action":"open_system","url":"https://example.com/article"}},"workspace_path":"{}"}}"#,
                    workspace_root
                ),
            })),
            runtime,
        ));

        let file_only_node_id = uhorse_protocol::NodeId::from_string("node-file-only");
        let (file_only_tx, mut file_only_rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(file_only_node_id.clone(), file_only_tx)
            .await;
        hub.handle_node_connection(
            file_only_node_id.clone(),
            "file-only-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
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

        let browser_node_id = uhorse_protocol::NodeId::from_string("node-browser-capable");
        let (browser_tx, mut browser_rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(browser_node_id.clone(), browser_tx)
            .await;
        hub.handle_node_connection(
            browser_node_id.clone(),
            "browser-node".to_string(),
            NodeCapabilities {
                supported_commands: vec![
                    CommandType::File,
                    CommandType::Shell,
                    CommandType::Code,
                    CommandType::Browser,
                ],
                ..NodeCapabilities::default()
            },
            WorkspaceInfo {
                workspace_id: None,
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
                MessageContent::Text("打开文章页面".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-browser".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        submit_dingtalk_task(&state, inbound).await.unwrap();

        let assignment = tokio::time::timeout(std::time::Duration::from_secs(1), browser_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let task_id = match assignment {
            HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                match command {
                    Command::Browser(BrowserCommand::OpenSystem { url }) => {
                        assert_eq!(url, "https://example.com/article");
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                assert_eq!(context.session_id.as_str(), "dingtalk:actual-user:corp-1");
                task_id
            }
            other => panic!("unexpected message: {:?}", other),
        };

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(200), file_only_rx.recv())
                .await
                .is_err()
        );

        let status = hub.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.status, TaskStatus::Running);
        assert_eq!(status.node_id.as_ref(), Some(&browser_node_id));

        hub.handle_node_message(
            &browser_node_id,
            NodeToHub::TaskResult {
                message_id: uhorse_protocol::MessageId::new(),
                task_id: task_id.clone(),
                result: CommandResult::success(CommandOutput::Browser {
                    result: BrowserResult::GetText {
                        text: "页面正文".to_string(),
                    },
                }),
                metrics: uhorse_protocol::ExecutionMetrics {
                    duration_ms: 1,
                    cpu_time_ms: 0,
                    peak_memory_mb: 0,
                    bytes_read: 0,
                    bytes_written: 0,
                    network_requests: 1,
                },
            },
        )
        .await
        .unwrap();

        let task_result =
            tokio::time::timeout(std::time::Duration::from_secs(1), task_result_rx.recv())
                .await
                .unwrap()
                .unwrap();
        assert_eq!(task_result.task_id, task_id);
        assert!(task_result.result.success);
        assert_eq!(format_task_result_message(&task_result.result), "页面正文");
        assert!(state.dingtalk_routes.read().await.contains_key(&task_id));

        let completed_task = hub.get_completed_task(&task_id).await.unwrap();
        assert_eq!(
            format_task_result_message(&completed_task.result),
            "页面正文"
        );

        reply_task_result(state.clone(), task_result).await.unwrap();
        assert!(state.dingtalk_routes.read().await.contains_key(&task_id));
    }

    #[test]
    fn test_resolve_dingtalk_reply_target_prefers_session_webhook() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-webhook".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: Some("https://example.com/hook".to_string()),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(
            resolve_dingtalk_reply_target(&route),
            Some(DingTalkReplyTarget::SessionWebhook {
                webhook: "https://example.com/hook".to_string(),
                at_user_ids: vec!["staff-1".to_string()],
            })
        );
    }

    #[test]
    fn test_resolve_dingtalk_reply_target_falls_back_to_group_message() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-group".to_string(),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: Some("https://example.com/hook".to_string()),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() - 1),
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(
            resolve_dingtalk_reply_target(&route),
            Some(DingTalkReplyTarget::GroupConversation {
                conversation_id: "conv-group".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_dingtalk_reply_target_uses_direct_user_for_private_chat() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-direct".to_string(),
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: None,
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(
            resolve_dingtalk_reply_target(&route),
            Some(DingTalkReplyTarget::DirectUser {
                user_id: "user-1".to_string(),
            })
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
                workspace_id: None,
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

        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session_state
                .metadata
                .get("collaboration_workspace_id")
                .map(String::as_str),
            Some("collab:session:dingtalk:actual-user:corp-1")
        );
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

        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session_state
                .metadata
                .get("collaboration_workspace_id")
                .map(String::as_str),
            Some("collab:session:dingtalk:actual-user:corp-1")
        );
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

        let session_key = SessionKey::new("dingtalk", "skill-user");
        let error = execute_local_skill(&state, &session_key, "disabled-skill", "input")
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

        let session_key = SessionKey::new("dingtalk", "skill-user");
        let error = execute_local_skill(&state, &session_key, "stderr-skill", "input")
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

        let session_key = SessionKey::new("dingtalk", "skill-user");
        let error = execute_local_skill(&state, &session_key, "timeout-skill", "input")
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

        let session_key = SessionKey::new("dingtalk", "skill-user");
        let output = execute_local_skill(&state, &session_key, "json-skill", "input")
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
        assert_eq!(skills[0].source_layer, "global");
        assert!(skills[0].source_scope.is_none());
    }

    #[tokio::test]
    async fn test_list_runtime_skills_expands_same_name_across_sources() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let user_scope = "user:dingtalk:user-shared";

        async fn write_skill(root: &std::path::Path, name: &str, script: &str) {
            let skill_dir = root.join("skills").join(name);
            tokio::fs::create_dir_all(&skill_dir).await.unwrap();
            tokio::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: {}\nversion: 1.0.0\ndescription: {} skill\nauthor: test\nparameters: []\npermissions: []\n---\n",
                    name, name
                ),
            )
            .await
            .unwrap();
            tokio::fs::write(
                skill_dir.join("skill.toml"),
                format!(
                    "enabled = true\ntimeout = 5\nexecutable = \"python3\"\nargs = [\"-c\", \"{}\"]\n",
                    script
                ),
            )
            .await
            .unwrap();
        }

        write_skill(&runtime_root, "echo", "print('global')").await;
        write_skill(
            &runtime_root.join("tenants").join(tenant_scope),
            "echo",
            "print('tenant')",
        )
        .await;
        write_skill(
            &runtime_root.join("users").join(user_scope),
            "echo",
            "print('user')",
        )
        .await;

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime,
        ));

        let Json(response) = list_runtime_skills(State(state)).await;
        let skills = response.data.unwrap();
        let echo_entries: Vec<_> = skills
            .into_iter()
            .filter(|skill| skill.name == "echo")
            .collect();
        assert_eq!(echo_entries.len(), 3);
        assert_eq!(echo_entries[0].source_layer, "global");
        assert!(echo_entries[0].source_scope.is_none());
        assert_eq!(echo_entries[1].source_layer, "tenant");
        assert_eq!(
            echo_entries[1].source_scope.as_deref(),
            Some("tenant:dingtalk:corp-shared")
        );
        assert_eq!(echo_entries[2].source_layer, "user");
        assert_eq!(
            echo_entries[2].source_scope.as_deref(),
            Some("user:dingtalk:user-shared")
        );
    }

    #[tokio::test]
    async fn test_get_runtime_skill_returns_requested_source_entry() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let user_scope = "user:dingtalk:user-shared";

        async fn write_skill(root: &std::path::Path, name: &str, script: &str) {
            let skill_dir = root.join("skills").join(name);
            tokio::fs::create_dir_all(&skill_dir).await.unwrap();
            tokio::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: {}\nversion: 1.0.0\ndescription: {} skill\nauthor: test\nparameters: []\npermissions: []\n---\n",
                    name, name
                ),
            )
            .await
            .unwrap();
            tokio::fs::write(
                skill_dir.join("skill.toml"),
                format!(
                    "enabled = true\ntimeout = 5\nexecutable = \"python3\"\nargs = [\"-c\", \"{}\"]\n",
                    script
                ),
            )
            .await
            .unwrap();
        }

        write_skill(&runtime_root, "echo", "print('global')").await;
        write_skill(
            &runtime_root.join("tenants").join(tenant_scope),
            "echo",
            "print('tenant')",
        )
        .await;
        write_skill(
            &runtime_root.join("users").join(user_scope),
            "echo",
            "print('user')",
        )
        .await;

        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let app = create_router(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime,
        ));

        let (status, body) = get_json(
            app,
            "/api/v1/skills/echo?source_layer=tenant&source_scope=tenant%3Adingtalk%3Acorp-shared",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["name"], json!("echo"));
        assert_eq!(body["data"]["source_layer"], json!("tenant"));
        assert_eq!(
            body["data"]["source_scope"],
            json!("tenant:dingtalk:corp-shared")
        );
    }

    #[tokio::test]
    async fn test_get_runtime_skill_returns_not_found_for_missing_source_entry() {
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
        let app = create_router((*state).clone());

        let (status, body) = get_json(
            app,
            "/api/v1/skills/echo?source_layer=user&source_scope=user%3Adingtalk%3Amissing",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["success"], json!(false));
        assert_eq!(body["error"], json!("Skill not found"));
    }

    #[tokio::test]
    async fn test_get_runtime_skill_returns_not_found_for_global_source_with_scope() {
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
        let app = create_router((*state).clone());

        let (status, body) = get_json(
            app,
            "/api/v1/skills/echo?source_layer=global&source_scope=tenant%3Adingtalk%3Acorp-shared",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["success"], json!(false));
        assert_eq!(body["error"], json!("Skill not found"));
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
            None,
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
        assert_eq!(
            sessions[0].namespace.as_ref().map(|ns| ns.global.as_str()),
            Some("global")
        );
        assert_eq!(
            sessions[0].namespace.as_ref().map(|ns| ns.user.as_str()),
            Some("user:dingtalk:user-api")
        );
        assert!(sessions[0]
            .namespace
            .as_ref()
            .and_then(|ns| ns.tenant.as_ref())
            .is_none());
        assert_eq!(
            sessions[0]
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.collaboration_workspace_id.as_str()),
            Some("collab:session:dingtalk:user-api")
        );
        assert_eq!(
            sessions[0]
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.scope_owner.as_str()),
            Some("session:dingtalk:user-api")
        );

        let (_, Json(session_detail_response)) =
            get_runtime_session(State(state.clone()), Path(session_key.as_str().to_string())).await;
        let session_detail = session_detail_response.data.unwrap();
        assert_eq!(
            session_detail.memory_context_chain,
            vec![
                "global".to_string(),
                "user:dingtalk:user-api".to_string(),
                "session:dingtalk:user-api".to_string()
            ]
        );
        assert_eq!(
            session_detail.visibility_chain,
            vec!["user:dingtalk:user-api".to_string(), "global".to_string()]
        );
        assert_eq!(
            session_detail
                .metadata
                .get("namespace_session")
                .map(String::as_str),
            Some("session:dingtalk:user-api")
        );
        assert_eq!(
            session_detail
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.collaboration_workspace_id.as_str()),
            Some("collab:session:dingtalk:user-api")
        );
        assert_eq!(
            session_detail
                .collaboration_workspace
                .as_ref()
                .map(|workspace| workspace.scope_owner.as_str()),
            Some("session:dingtalk:user-api")
        );

        let (_, Json(messages_response)) =
            get_runtime_session_messages(State(state), Path(session_key.as_str().to_string()))
                .await;
        let messages = messages_response.data.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].user_message, "hello");
        assert_eq!(messages[0].assistant_message, "world");
    }

    #[tokio::test]
    async fn test_get_runtime_session_messages_uses_session_history_not_catalog_scope() {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        let tenant_scope = "tenant:dingtalk:corp-shared";
        let user_scope = "user:dingtalk:user-shared";
        let tenant_root = runtime_root.join("tenants").join(tenant_scope);
        let user_root = runtime_root.join("users").join(user_scope);
        tokio::fs::create_dir_all(tenant_root.join("workspace-helper"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(user_root.join("workspace-helper"))
            .await
            .unwrap();

        let runtime = Arc::new(
            init_default_agent_runtime(runtime_root.clone())
                .await
                .unwrap(),
        );
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::with_team("dingtalk", "user-shared", "corp-shared");

        persist_session_state(
            &state,
            &session_key,
            "helper",
            "conv-shared",
            Some("user-shared"),
            None,
            None,
            None,
            None,
        )
        .await;

        runtime
            .memory_store
            .store_message(
                &CoreSessionId::from_string(session_key.as_str()),
                "from layered memory",
                "session reply",
            )
            .await
            .unwrap();

        let tenant_history_path = runtime_root
            .join("tenants")
            .join(tenant_scope)
            .join("workspace-helper")
            .join("sessions")
            .join(session_key.as_str())
            .join("history.md");
        tokio::fs::create_dir_all(tenant_history_path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(
            &tenant_history_path,
            "## 2026-03-26 00:00:00 UTC\n\n**User:** tenant scope\n\n**Assistant:** tenant reply\n\n",
        )
        .await
        .unwrap();

        let (_, Json(messages_response)) =
            get_runtime_session_messages(State(state), Path(session_key.as_str().to_string()))
                .await;
        let messages = messages_response.data.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].user_message, "from layered memory");
        assert_eq!(messages[0].assistant_message, "session reply");
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
    async fn test_list_nodes_api_includes_workspace_id() {
        let (app, _state, _hub, node_id, _rx, _workspace) =
            create_router_test_state_with_registered_node().await;

        let (status, body) = get_json(app, "/api/nodes").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"][0]["node_id"], json!(node_id.as_str()));
        assert_eq!(
            body["data"][0]["workspace"]["workspace_id"],
            json!(format!("exec:{}:workspace", node_id.as_str()))
        );
    }

    #[tokio::test]
    async fn test_get_node_api_includes_workspace_id() {
        let (app, _state, _hub, node_id, _rx, _workspace) =
            create_router_test_state_with_registered_node().await;

        let (status, body) = get_json(app, &format!("/api/nodes/{}", node_id.as_str())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["node_id"], json!(node_id.as_str()));
        assert_eq!(
            body["data"]["workspace"]["workspace_id"],
            json!(format!("exec:{}:workspace", node_id.as_str()))
        );
    }

    #[tokio::test]
    async fn test_submit_task_api_dispatches_assignment_and_persists_task_status() {
        let (app, hub, node_id, mut rx, workspace) = create_task_submit_test_state().await;
        let target_path = workspace.path().join("Cargo.toml");
        let execution_workspace_id = format!("exec:{}:workspace", node_id.as_str());
        let collaboration_workspace_id = "collab:web-session";
        let serialized_roles = serde_json::to_string(&vec!["role:web-org:manager"]).unwrap();

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
                "session_id": "api:api-user",
                "channel": "api",
                "execution_workspace_id": execution_workspace_id,
                "collaboration_workspace_id": collaboration_workspace_id,
                "intent": "check file",
                "env": {
                    "source": "web-test",
                    "namespace_enterprise": "enterprise:web-org",
                    "namespace_roles": serialized_roles.clone()
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

        let (status, task_body) = get_json(
            create_router(WebState::new(hub.clone(), None, None)),
            &format!("/api/tasks/{}", task_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(task_body["success"], json!(true));
        assert_eq!(
            task_body["data"]["execution_workspace_id"],
            json!(execution_workspace_id)
        );
        assert_eq!(
            task_body["data"]["collaboration_workspace"]["collaboration_workspace_id"],
            json!(collaboration_workspace_id)
        );
        assert_eq!(
            task_body["data"]["collaboration_workspace"]["scope_owner"],
            json!("session:api:api-user")
        );

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
                assert_eq!(context.session_id.as_str(), "api:api-user");
                assert_eq!(context.channel, "api");
                assert_eq!(
                    context.execution_workspace_id.as_deref(),
                    Some(execution_workspace_id.as_str())
                );
                assert_eq!(
                    context.collaboration_workspace_id.as_deref(),
                    Some(collaboration_workspace_id)
                );
                assert_eq!(
                    context
                        .env
                        .get("execution_workspace_id")
                        .map(String::as_str),
                    Some(execution_workspace_id.as_str())
                );
                assert_eq!(
                    context
                        .env
                        .get("collaboration_workspace_id")
                        .map(String::as_str),
                    Some(collaboration_workspace_id)
                );
                assert_eq!(
                    context
                        .env
                        .get("collaboration_scope_owner")
                        .map(String::as_str),
                    Some("session:api:api-user")
                );
                assert_eq!(
                    context
                        .env
                        .get("collaboration_materialization")
                        .map(String::as_str),
                    Some("none")
                );
                assert_eq!(context.intent.as_deref(), Some("check file"));
                assert_eq!(
                    context.env.get("source").map(String::as_str),
                    Some("web-test")
                );
                assert_eq!(
                    context.env.get("namespace_global").map(String::as_str),
                    Some("global")
                );
                assert_eq!(
                    context.env.get("namespace_enterprise").map(String::as_str),
                    Some("enterprise:web-org")
                );
                assert_eq!(
                    context.env.get("namespace_roles").map(String::as_str),
                    Some(serialized_roles.as_str())
                );
                assert_eq!(
                    context.env.get("namespace_user").map(String::as_str),
                    Some("user:api:api-user")
                );
                assert_eq!(
                    context.env.get("namespace_session").map(String::as_str),
                    Some("session:api:api-user")
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
    fn test_validate_browser_command_allows_public_https_url() {
        let command = uhorse_protocol::BrowserCommand::Navigate {
            url: "https://example.com/article".to_string(),
        };
        assert!(validate_browser_command(&command).is_ok());
    }

    #[test]
    fn test_validate_browser_command_allows_open_system_public_https_url() {
        let command = uhorse_protocol::BrowserCommand::OpenSystem {
            url: "https://example.com/article".to_string(),
        };
        assert!(validate_browser_command(&command).is_ok());
    }

    #[test]
    fn test_validate_browser_command_rejects_localhost() {
        let command = uhorse_protocol::BrowserCommand::Navigate {
            url: "http://localhost:3000".to_string(),
        };
        assert!(validate_browser_command(&command).is_err());
    }

    #[test]
    fn test_validate_browser_command_rejects_private_ip() {
        let command = uhorse_protocol::BrowserCommand::Navigate {
            url: "http://192.168.1.10/internal".to_string(),
        };
        assert!(validate_browser_command(&command).is_err());
    }

    #[test]
    fn test_validate_browser_command_rejects_file_scheme() {
        let command = uhorse_protocol::BrowserCommand::Navigate {
            url: "file:///tmp/demo.html".to_string(),
        };
        assert!(validate_browser_command(&command).is_err());
    }

    #[test]
    fn test_format_task_result_message_browser_open_system() {
        let result = CommandResult::success(CommandOutput::Browser {
            result: BrowserResult::OpenSystem {
                url: "https://example.com/article".to_string(),
            },
        });
        assert_eq!(
            format_task_result_message(&result),
            "已在系统浏览器打开：https://example.com/article"
        );
    }

    #[test]
    fn test_format_task_result_message_browser_navigate_with_title() {
        let result = CommandResult::success(CommandOutput::Browser {
            result: BrowserResult::Navigate {
                final_url: "https://example.com/article".to_string(),
                title: Some("Example Article".to_string()),
            },
        });
        assert_eq!(
            format_task_result_message(&result),
            "浏览器会话已导航到：https://example.com/article\n标题：Example Article"
        );
    }

    #[test]
    fn test_format_task_result_message_browser_text() {
        let result = CommandResult::success(CommandOutput::Browser {
            result: BrowserResult::GetText {
                text: "页面正文".to_string(),
            },
        });
        assert_eq!(format_task_result_message(&result), "页面正文");
    }

    #[test]
    fn test_format_task_result_message_file_write() {
        let result = CommandResult::success(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "write",
            "path": "/tmp/demo.md",
            "bytes_written": 42,
        })));
        assert_eq!(
            format_task_result_message(&result),
            "已保存成功：/tmp/demo.md"
        );
    }

    #[test]
    fn test_format_task_result_message_file_copy() {
        let result = CommandResult::success(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "copy",
            "source_path": "/tmp/source.md",
            "destination_path": "/tmp/dest.md",
            "bytes_copied": 42,
        })));
        assert_eq!(
            format_task_result_message(&result),
            "已复制成功：/tmp/source.md\n到：/tmp/dest.md"
        );
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
        assert_eq!(
            body["data"]["collaboration_workspace"]["default_agent_id"],
            json!(null)
        );
        assert_eq!(
            body["data"]["collaboration_workspace"]["bound_execution_workspace_id"],
            json!(null)
        );
    }
}
