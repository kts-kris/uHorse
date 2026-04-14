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
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Cursor;
use std::net::IpAddr;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tar::Archive;
use zip::ZipArchive;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use uhorse_agent::{
    scope_layer_from_scope, scope_layer_rank, AccessContext, Agent, AgentManager, AgentScope,
    AgentScopeConfig, LayeredMemoryStore, LayeredSkillEntry, LayeredSkillRegistry, MemoryStore,
    SessionKey, SessionNamespace, SessionState, SkillRegistry,
};
use uhorse_channel::{
    dingtalk::{
        DingTalkAiCardHandle, DingTalkAiCardTarget, DingTalkEvent, DingTalkInboundAttachment,
        DingTalkReactionHandle, DingTalkTransientClearOutcome, DingTalkTransientMessageReceipt,
    },
    DingTalkChannel, DingTalkInboundMessage,
};
use uhorse_config::{DingTalkSkillInstaller, HealthConfig, UHorseConfig};
use uhorse_core::{Channel, MessageContent, SessionId as CoreSessionId};
use uhorse_llm::{ChatMessage, LLMClient};
use uhorse_multimodal::stt::{SttClient, SttConfig};
use uhorse_observability::{
    log_audit_event, AuditCategory, AuditEvent, AuditLevel, HealthService, HealthStatus,
    MetricsCollector, MetricsExporter,
};
use uhorse_protocol::{
    BrowserResult, Command, CommandOutput, CommandType, FileCommand, HubToNode, MessageId,
    NodeCapabilities, PermissionRule as ProtocolPermissionRule, Priority, SessionId, TaskContext,
    TaskId, UserId,
};
use uhorse_security::{ApprovalRequest, DevicePairingManager, PairingRequest, PairingStatus};

use crate::{
    node_manager::workspace_matches_hint,
    session_runtime::{SessionMailboxSnapshot, TaskContinuationBinding, TranscriptEventKind, TurnStatus},
    task_scheduler::{CompletedTask, TaskResult},
    Hub, HubStats,
};
pub use ws::ws_handler;

/// DingTalk 回传路由
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkReplyRoute {
    /// 会话 ID
    pub conversation_id: String,
    /// 原始消息 ID
    pub source_message_id: Option<String>,
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

const DINGTALK_PROCESSING_ACK_TEXT: &str = "[Wait]";
const DINGTALK_CANCELLED_TEXT: &str = "任务已取消。";
const BEARER_AUTH_PREFIX: &str = "Bearer ";
const DINGTALK_DIRECT_REPLY_MIN_ACK_DISPLAY_MILLIS: u64 = 900;
const DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY: &str = "pending_attachment_context";
const DINGTALK_LAST_AUDIO_TRANSCRIPT_KEY: &str = "last_audio_transcript";
const DINGTALK_ATTACHMENT_WAITING_REPLY_TEXT: &str = "已收到这条消息，请继续告诉我你希望我怎么处理。";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PendingDingTalkAttachment {
    kind: String,
    summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    download_code: Option<String>,
}

#[derive(Debug, Clone)]
enum NormalizedDingTalkInbound {
    ContinueAsText {
        text: String,
        consumed_pending_attachments: Vec<PendingDingTalkAttachment>,
    },
    WaitForFollowUp { reply_text: String },
}

#[derive(Debug, Clone)]
enum DingTalkReplyHandle {
    AiCard {
        handle: DingTalkAiCardHandle,
        attached_at: Instant,
    },
    Reaction {
        handle: DingTalkReactionHandle,
        attached_at: Instant,
    },
    LegacyTransient {
        receipt: Option<DingTalkTransientMessageReceipt>,
        attached_at: Instant,
    },
    Noop,
}

impl DingTalkReplyHandle {
    fn attached_at(&self) -> Option<Instant> {
        match self {
            Self::AiCard { attached_at, .. }
            | Self::Reaction { attached_at, .. }
            | Self::LegacyTransient { attached_at, .. } => Some(*attached_at),
            Self::Noop => None,
        }
    }
}

const DEFAULT_SKILLHUB_SEARCH_URL: &str =
    "https://api.skillhub.tencent.com/api/skills";
const SKILLHUB_PRIMARY_DOWNLOAD_URL_TEMPLATE: &str =
    "https://api.skillhub.tencent.com/api/v1/download?slug={slug}";
const SKILLHUB_HTTP_TIMEOUT_SECS: u64 = 15;
const SKILLHUB_OFFICIAL_HOSTS: [&str; 2] = [
    "skillhub-1388575217.cos.ap-guangzhou.myqcloud.com",
    "api.skillhub.tencent.com",
];

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
    ListInstalledSkills,
    QuerySkill {
        skill_name: String,
    },
    InstallSkill {
        query: String,
    },
}

#[derive(Debug, Clone)]
enum PlannedTurnStep {
    Finalize {
        text: String,
    },
    SubmitTask {
        command: Command,
        workspace_path: Option<String>,
    },
    ExecuteSkill {
        skill_name: String,
        input: String,
    },
    ListInstalledSkills,
    QuerySkill {
        skill_name: String,
    },
    InstallSkill {
        query: String,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SkillInstallSourceType {
    Skillhub,
    DingtalkAttachment,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SkillInstallTargetLayer {
    Global,
    User,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillInstallRequest {
    source_type: SkillInstallSourceType,
    package: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    download_url: String,
    #[serde(default = "default_skill_install_target_layer")]
    target_layer: SkillInstallTargetLayer,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    attachment_download_code: Option<String>,
    #[serde(default)]
    attachment_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SkillInstallResponse {
    skill_name: String,
    source_type: String,
    package: String,
    version: Option<String>,
    target_layer: String,
    target_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SkillRefreshResponse {
    skill_count: usize,
}

#[derive(Debug, Clone)]
struct SkillInstallActor {
    channel: &'static str,
    sender_user_id: Option<String>,
    sender_staff_id: Option<String>,
    sender_corp_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SkillhubSearchEntry {
    slug: String,
    name: String,
    version: Option<String>,
}

#[derive(Debug, Clone)]
enum DingtalkSkillInstallIntent {
    ExplicitCommand(SkillInstallRequest),
    NaturalLanguage(SkillhubSearchEntry),
    NaturalLanguageNoMatch(String),
}

fn default_skill_install_target_layer() -> SkillInstallTargetLayer {
    SkillInstallTargetLayer::Global
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
    runtime_mailbox: Option<SessionMailboxSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct HubRuntimeDiagnosticsResponse {
    session_mailboxes: Vec<SessionMailboxSnapshot>,
    waiting_for_tool_turns: usize,
    waiting_for_approval_turns: usize,
    active_task_bindings: usize,
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
    /// Agent 运行时根目录
    pub runtime_root: PathBuf,
    /// Agent 管理器
    pub agent_manager: Arc<AgentManager>,
    /// 分层 Agent catalog
    pub agents: Arc<LayeredAgentCatalog>,
    /// Memory 存储
    pub memory_store: Arc<LayeredMemoryStore>,
    /// 分层 Skill 注册表
    pub skills: Arc<RwLock<LayeredSkillRegistry>>,
}

/// Web 服务器状态
#[derive(Clone)]
pub struct WebState {
    /// 应用配置
    pub app_config: Arc<UHorseConfig>,
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
    /// 设备绑定管理器
    pub pairing_manager: Option<Arc<DevicePairingManager>>,
    /// Agent 运行时
    pub agent_runtime: Arc<WebAgentRuntime>,
    /// 任务回传路由
    pub dingtalk_routes: Arc<RwLock<HashMap<TaskId, DingTalkReplyRoute>>>,
    /// 异步任务处理中提示句柄
    dingtalk_reply_handles: Arc<RwLock<HashMap<TaskId, DingTalkReplyHandle>>>,
}

impl WebState {
    /// 创建新的 Web 状态
    pub fn new(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
    ) -> Self {
        Self::new_with_pairing(hub, dingtalk_channel, llm_client, None)
    }

    /// 使用显式应用配置创建 Web 状态。
    pub fn new_with_config(
        app_config: Arc<UHorseConfig>,
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
    ) -> Self {
        Self::new_with_pairing_and_config(app_config, hub, dingtalk_channel, llm_client, None)
    }

    /// 创建带设备绑定管理器的 Web 状态
    pub fn new_with_pairing(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        pairing_manager: Option<Arc<DevicePairingManager>>,
    ) -> Self {
        Self::new_with_pairing_and_config(
            Arc::new(UHorseConfig::default()),
            hub,
            dingtalk_channel,
            llm_client,
            pairing_manager,
        )
    }

    /// 使用显式应用配置与设备绑定管理器创建 Web 状态。
    pub fn new_with_pairing_and_config(
        app_config: Arc<UHorseConfig>,
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        pairing_manager: Option<Arc<DevicePairingManager>>,
    ) -> Self {
        Self::new_with_runtime_and_config(
            app_config,
            hub,
            dingtalk_channel,
            llm_client,
            pairing_manager,
            Arc::new(default_agent_runtime()),
        )
    }

    /// 使用指定 Agent 运行时创建 Web 状态
    pub fn new_with_runtime(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        pairing_manager: Option<Arc<DevicePairingManager>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        Self::new_with_runtime_and_config(
            Arc::new(UHorseConfig::default()),
            hub,
            dingtalk_channel,
            llm_client,
            pairing_manager,
            agent_runtime,
        )
    }

    /// 使用显式应用配置与 Agent 运行时创建 Web 状态。
    pub fn new_with_runtime_and_config(
        app_config: Arc<UHorseConfig>,
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        pairing_manager: Option<Arc<DevicePairingManager>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        let metrics_collector = Arc::new(MetricsCollector::default());
        let metrics_exporter = Arc::new(MetricsExporter::new(Arc::clone(&metrics_collector)));
        Self::new_with_runtime_and_health_and_config(
            app_config,
            hub,
            Arc::new(HealthService::new(env!("CARGO_PKG_VERSION").to_string())),
            metrics_collector,
            metrics_exporter,
            dingtalk_channel,
            llm_client,
            pairing_manager,
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
        pairing_manager: Option<Arc<DevicePairingManager>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        Self::new_with_runtime_and_health_and_config(
            Arc::new(UHorseConfig::default()),
            hub,
            health_service,
            metrics_collector,
            metrics_exporter,
            dingtalk_channel,
            llm_client,
            pairing_manager,
            agent_runtime,
        )
    }

    /// 使用显式应用配置、health 与 metrics 依赖创建 Web 状态。
    pub fn new_with_runtime_and_health_and_config(
        app_config: Arc<UHorseConfig>,
        hub: Arc<Hub>,
        health_service: Arc<HealthService>,
        metrics_collector: Arc<MetricsCollector>,
        metrics_exporter: Arc<MetricsExporter>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<dyn LLMClient>>,
        pairing_manager: Option<Arc<DevicePairingManager>>,
        agent_runtime: Arc<WebAgentRuntime>,
    ) -> Self {
        Self {
            app_config,
            hub,
            health_service,
            metrics_collector,
            metrics_exporter,
            dingtalk_channel,
            llm_client,
            pairing_manager,
            agent_runtime,
            dingtalk_routes: Arc::new(RwLock::new(HashMap::new())),
            dingtalk_reply_handles: Arc::new(RwLock::new(HashMap::new())),
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

    let runtime_root = agent_manager.base_dir().to_path_buf();

    WebAgentRuntime {
        runtime_root,
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(LayeredAgentCatalog::default()),
        memory_store,
        skills: Arc::new(RwLock::new(LayeredSkillRegistry::new(SkillRegistry::new()))),
    }
}

async fn load_runtime_skills(
    base_dir: &FsPath,
) -> Result<LayeredSkillRegistry, Box<dyn std::error::Error + Send + Sync>> {
    let global_skills = SkillRegistry::from_dir(base_dir.join("skills")).await?;
    let mut layered_skills = LayeredSkillRegistry::new(global_skills);

    async fn load_scoped_runtime_dir(
        base_dir: &FsPath,
        dir_name: &str,
        layered_skills: &mut LayeredSkillRegistry,
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
        }

        Ok(())
    }

    load_scoped_runtime_dir(base_dir, "tenants", &mut layered_skills).await?;
    load_scoped_runtime_dir(base_dir, "enterprises", &mut layered_skills).await?;
    load_scoped_runtime_dir(base_dir, "departments", &mut layered_skills).await?;
    load_scoped_runtime_dir(base_dir, "roles", &mut layered_skills).await?;
    load_scoped_runtime_dir(base_dir, "users", &mut layered_skills).await?;

    Ok(layered_skills)
}

async fn refresh_runtime_skills(
    runtime: &WebAgentRuntime,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let refreshed = load_runtime_skills(&runtime.runtime_root).await?;
    let skill_count = refreshed.list_all_entries().len();
    *runtime.skills.write().await = refreshed;
    Ok(skill_count)
}

fn dingtalk_skill_installers(state: &WebState) -> &[DingTalkSkillInstaller] {
    state
        .app_config
        .channels
        .dingtalk
        .as_ref()
        .map(|config| config.skill_installers.as_slice())
        .unwrap_or(&[])
}

fn actor_can_install_skill(state: &WebState, actor: &SkillInstallActor) -> bool {
    if actor.channel != "dingtalk" {
        return true;
    }

    let installers = dingtalk_skill_installers(state);
    if installers.is_empty() {
        return false;
    }

    installers.iter().any(|installer| {
        if let Some(corp_id) = installer.corp_id.as_deref() {
            if actor.sender_corp_id.as_deref() != Some(corp_id) {
                return false;
            }
        }

        let user_matches = installer
            .user_id
            .as_deref()
            .is_some_and(|user_id| actor.sender_user_id.as_deref() == Some(user_id));
        let staff_matches = installer
            .staff_id
            .as_deref()
            .is_some_and(|staff_id| actor.sender_staff_id.as_deref() == Some(staff_id));

        user_matches || staff_matches
    })
}

fn resolve_skill_install_target_dir(
    runtime: &WebAgentRuntime,
    target_layer: &SkillInstallTargetLayer,
    target_scope: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    match target_layer {
        SkillInstallTargetLayer::Global => {
            if target_scope.is_some() {
                return Err("global 安装不允许传 target_scope".into());
            }
            Ok(runtime.runtime_root.join("skills"))
        }
        SkillInstallTargetLayer::User => {
            let scope = target_scope
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "user 安装必须提供 target_scope".to_string())?;
            Ok(runtime.runtime_root.join("users").join(scope).join("skills"))
        }
    }
}

fn build_skillhub_http_client() -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(SKILLHUB_HTTP_TIMEOUT_SECS))
        .build()?)
}

async fn fetch_skillhub_archive(
    client: &reqwest::Client,
    request: &SkillInstallRequest,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(&request.download_url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Skill 下载失败：HTTP {}", status).into());
    }
    response.bytes().await.map(|bytes| bytes.to_vec()).map_err(|error| {
        format!(
            "Skill 下载响应解析失败：url={} status={} error={}",
            request.download_url, status, error
        )
        .into()
    })
}

async fn fetch_dingtalk_attachment_archive(
    state: &Arc<WebState>,
    request: &SkillInstallRequest,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let download_code = request
        .attachment_download_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("缺少钉钉附件下载凭证")?;
    let file_name = request
        .attachment_file_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("uploaded-skill.zip")
        .to_string();
    let channel = state
        .dingtalk_channel
        .as_ref()
        .ok_or("DingTalk channel is unavailable")?;
    let bytes = channel
        .download_inbound_attachment(&DingTalkInboundAttachment {
            kind: "file".to_string(),
            key: None,
            file_name: Some(file_name),
            download_code: Some(download_code.to_string()),
            recognition: None,
            caption: None,
        })
        .await?;
    Ok(bytes)
}

async fn unpack_skill_archive(
    bytes: &[u8],
    destination_root: &FsPath,
    package_hint: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let destination_root = destination_root.to_path_buf();
    let bytes = bytes.to_vec();
    let package_hint = package_hint.to_string();
    tokio::task::spawn_blocking(
        move || -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            fn collect_skill_dir(root: &FsPath) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
                let mut skill_dirs = Vec::new();
                for entry in std::fs::read_dir(root)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        let skill_md = entry.path().join("SKILL.md");
                        if skill_md.exists() {
                            skill_dirs.push(entry.path());
                        }
                    }
                }

                if skill_dirs.len() != 1 {
                    return Err("Skill 安装包必须且只能包含一个 Skill 目录".into());
                }

                let skill_dir = skill_dirs.remove(0);
                Ok(skill_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| "Skill 目录名称非法".to_string())?
                    .to_string())
            }

            std::fs::create_dir_all(&destination_root)?;

            if bytes.starts_with(b"PK\x03\x04") {
                let skill_root = destination_root.join(&package_hint);
                std::fs::create_dir_all(&skill_root)?;
                let reader = Cursor::new(bytes);
                let mut archive = ZipArchive::new(reader)?;
                for index in 0..archive.len() {
                    let mut file = archive.by_index(index)?;
                    let Some(path) = file.enclosed_name().map(|path| path.to_path_buf()) else {
                        continue;
                    };
                    let output_path = skill_root.join(path);
                    if file.is_dir() {
                        std::fs::create_dir_all(&output_path)?;
                        continue;
                    }
                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut output = std::fs::File::create(&output_path)?;
                    std::io::copy(&mut file, &mut output)?;
                }

                if skill_root.join("SKILL.md").exists() {
                    return Ok(package_hint);
                }

                let nested_skill_name = collect_skill_dir(&skill_root)?;
                let nested_skill_root = skill_root.join(&nested_skill_name);
                if nested_skill_name == package_hint {
                    for entry in std::fs::read_dir(&nested_skill_root)? {
                        let entry = entry?;
                        let target = skill_root.join(entry.file_name());
                        std::fs::rename(entry.path(), target)?;
                    }
                    std::fs::remove_dir_all(&nested_skill_root)?;
                    return Ok(package_hint);
                }

                let final_skill_root = destination_root.join(&nested_skill_name);
                if final_skill_root.exists() {
                    std::fs::remove_dir_all(&final_skill_root)?;
                }
                std::fs::rename(&nested_skill_root, &final_skill_root)?;
                std::fs::remove_dir_all(&skill_root)?;
                return Ok(nested_skill_name);
            }

            if !bytes.starts_with(&[0x1f, 0x8b]) {
                return Err("Skill 安装包格式非法：既不是 zip，也不是 tar.gz".into());
            }

            let decoder = GzDecoder::new(Cursor::new(bytes));
            let mut archive = Archive::new(decoder);
            archive.unpack(&destination_root)?;
            collect_skill_dir(&destination_root)
        },
    )
    .await?
}

fn build_skill_install_dir(runtime_root: &FsPath, result: &SkillInstallResponse) -> PathBuf {
    match result.target_layer.as_str() {
        "user" => runtime_root
            .join("users")
            .join(result.target_scope.as_deref().unwrap_or_default())
            .join("skills")
            .join(&result.skill_name),
        _ => runtime_root.join("skills").join(&result.skill_name),
    }
}

async fn ensure_generated_skill_toml_if_missing(
    skill_dir: &FsPath,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let skill_toml_path = skill_dir.join("skill.toml");
    if skill_toml_path.exists() {
        return Ok(());
    }

    if !skill_dir.join("skill.yaml").exists() {
        return Ok(());
    }

    let entrypoint = if skill_dir.join("src").join("main.py").exists() {
        Some("src/main.py")
    } else if skill_dir.join("main.py").exists() {
        Some("main.py")
    } else {
        None
    };

    let Some(entrypoint) = entrypoint else {
        return Ok(());
    };

    let venv_python = skill_dir.join(".venv").join("bin").join("python3");
    let executable = if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        "python3".to_string()
    };

    let mut generated = String::new();
    let _ = writeln!(&mut generated, "enabled = true");
    let _ = writeln!(&mut generated, "timeout = 30");
    let _ = writeln!(&mut generated, "executable = \"{}\"", executable.replace('\\', "\\\\"));
    let _ = writeln!(&mut generated, "args = [\"{}\"]", entrypoint);
    tokio::fs::write(&skill_toml_path, generated).await?;
    Ok(())
}

async fn install_python_skill_dependencies(
    skill_dir: &FsPath,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let requirements_path = skill_dir.join("requirements.txt");
    if !requirements_path.exists() {
        return Ok(());
    }

    let venv_dir = skill_dir.join(".venv");
    let venv_python = venv_dir.join("bin").join("python3");
    let venv_pip = venv_dir.join("bin").join("pip");

    if !venv_python.exists() {
        let output = tokio::process::Command::new("python3")
            .arg("-m")
            .arg("venv")
            .arg(&venv_dir)
            .current_dir(skill_dir)
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(format!("创建 Skill Python 虚拟环境失败：{}", detail).into());
        }
    }

    let output = tokio::process::Command::new(&venv_pip)
        .arg("install")
        .arg("-r")
        .arg(&requirements_path)
        .current_dir(skill_dir)
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(format!("安装 Skill Python 依赖失败：{}", detail).into());
    }

    Ok(())
}

fn extract_skill_manifest_description(skill_md: &str) -> Option<String> {
    let frontmatter = skill_md.splitn(3, "---").nth(1)?.trim();
    frontmatter
        .lines()
        .find_map(|line| line.trim().strip_prefix("description:"))
        .map(|value| value.trim().trim_matches('"').trim_matches('\'').trim_end_matches('。').to_string())
        .filter(|value| !value.is_empty())
}

fn build_skill_install_example_in_chinese(
    skill_name: &str,
    description: &str,
    usage_hint: Option<&str>,
) -> String {
    let lower_name = skill_name.to_ascii_lowercase();
    let lower_description = description.to_ascii_lowercase();
    let lower_usage_hint = usage_hint.unwrap_or_default().to_ascii_lowercase();
    let browser_related = ["browser", "web", "网页", "表单", "截图", "自动化"]
        .iter()
        .any(|keyword| {
            lower_name.contains(keyword)
                || lower_description.contains(keyword)
                || lower_usage_hint.contains(keyword)
        });

    if browser_related {
        "请打开目标网页，帮我完成点击、输入或截图，并把结果发给我".to_string()
    } else {
        format!("请使用 {} 帮我处理这个任务", skill_name)
    }
}

fn build_skill_install_summary_messages(skill_name: &str, skill_md: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(
            "你是 uHorse Hub 的 Skill 安装提示生成器。请基于提供的 SKILL.md 内容，只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。JSON 结构必须是 {\"description_zh\":\"...\",\"example_zh\":\"...\"}。要求：1）两个字段都必须是简体中文；2）description_zh 用一句话概括 Skill 能力；3）example_zh 必须是用户可以直接发送给机器人的自然语言请求示例，不能是命令、不能夹杂英文、不能用反引号；4）如果是浏览器/网页自动化相关 Skill，示例应贴近打开网页、点击、输入、截图、抓取信息等场景；5）不要照抄英文原文。".to_string(),
        ),
        ChatMessage::user(format!(
            "skill_name: {}\nSKILL.md:\n{}\n请输出单个 JSON 对象。",
            skill_name, skill_md
        )),
    ]
}

#[derive(Debug, Deserialize)]
struct SkillInstallHintSummary {
    description_zh: String,
    example_zh: String,
}

async fn summarize_skill_install_hint_in_chinese(
    state: &Arc<WebState>,
    skill_name: &str,
    skill_md: &str,
) -> Option<SkillInstallHintSummary> {
    let llm_client = state.llm_client.as_ref()?;
    let response = llm_client
        .chat_completion(build_skill_install_summary_messages(skill_name, skill_md))
        .await
        .ok()?;
    let summary: SkillInstallHintSummary = serde_json::from_str(&response).ok()?;
    let description_zh = summary.description_zh.trim().trim_end_matches('。').to_string();
    let example_zh = summary.example_zh.trim().trim_end_matches('。').to_string();
    if description_zh.is_empty() || example_zh.is_empty() {
        return None;
    }
    Some(SkillInstallHintSummary {
        description_zh,
        example_zh,
    })
}

async fn build_skill_install_trigger_hint(
    state: &Arc<WebState>,
    result: &SkillInstallResponse,
) -> String {
    let exact_entry = {
        let skills = state.agent_runtime.skills.read().await;
        skills
            .get_entry_by_source(
                &result.skill_name,
                &result.target_layer,
                result.target_scope.as_deref(),
            )
            .or_else(|| skills.get_any_entry(&result.skill_name))
    };
    let manifest_description = exact_entry
        .as_ref()
        .map(|entry| entry.skill.manifest.description.clone());
    let skill_md_path = build_skill_install_dir(&state.agent_runtime.runtime_root, result).join("SKILL.md");
    let skill_md = tokio::fs::read_to_string(&skill_md_path).await.ok();
    let description = skill_md
        .as_deref()
        .and_then(extract_skill_manifest_description)
        .or(manifest_description)
        .unwrap_or_else(|| "帮助你完成特定任务".to_string());
    let usage_hint = skill_md.as_deref().and_then(extract_skill_usage_hint);

    let (description_zh, chinese_example) = match skill_md.as_deref() {
        Some(skill_md) => summarize_skill_install_hint_in_chinese(state, &result.skill_name, skill_md)
            .await
            .map(|summary| (summary.description_zh, summary.example_zh))
            .unwrap_or_else(|| {
                (
                    description.clone(),
                    build_skill_install_example_in_chinese(
                        &result.skill_name,
                        &description,
                        usage_hint.as_deref(),
                    ),
                )
            }),
        None => (
            description.clone(),
            build_skill_install_example_in_chinese(
                &result.skill_name,
                &description,
                usage_hint.as_deref(),
            ),
        ),
    };

    format!(
        "这个 Skill 主要用于：{}。你现在可以直接用自然语言描述需求，例如：{}。",
        description_zh, chinese_example
    )
}

fn extract_skill_usage_hint(skill_md: &str) -> Option<String> {
    let body = skill_md.splitn(3, "---").nth(2)?.trim();
    for line in body.lines() {
        let text = line.trim();
        if text.is_empty() || text.starts_with('#') || text.starts_with("```") {
            continue;
        }
        if text.starts_with('-') || text.starts_with('*') {
            let bullet = text[1..].trim();
            if !bullet.is_empty() {
                return Some(bullet.trim_end_matches('。').to_string());
            }
            continue;
        }
        return Some(text.trim_end_matches('。').to_string());
    }
    None
}

async fn install_skill_from_request(
    state: &Arc<WebState>,
    actor: SkillInstallActor,
    request: SkillInstallRequest,
) -> Result<SkillInstallResponse, Box<dyn std::error::Error + Send + Sync>> {
    let source_type_label = match request.source_type {
        SkillInstallSourceType::Skillhub => "skillhub",
        SkillInstallSourceType::DingtalkAttachment => "dingtalk_attachment",
    };
    if !actor_can_install_skill(state.as_ref(), &actor) {
        info!(
            action = "skill_install",
            channel = actor.channel,
            sender_user_id = actor.sender_user_id.as_deref().unwrap_or(""),
            sender_staff_id = actor.sender_staff_id.as_deref().unwrap_or(""),
            sender_corp_id = actor.sender_corp_id.as_deref().unwrap_or(""),
            package = request.package.as_str(),
            target_layer = match request.target_layer { SkillInstallTargetLayer::Global => "global", SkillInstallTargetLayer::User => "user" },
            result = "denied",
            "Denied skill installation request"
        );
        let _ = log_audit_event(AuditEvent {
            timestamp: chrono::Utc::now().timestamp() as u64,
            level: AuditLevel::Warn,
            category: AuditCategory::Tool,
            actor: actor.sender_user_id.clone().or(actor.sender_staff_id.clone()),
            action: "skill_install_denied".to_string(),
            target: Some(request.package.clone()),
            details: Some(serde_json::json!({
                "channel": actor.channel,
                "source_type": source_type_label,
                "target_layer": match request.target_layer { SkillInstallTargetLayer::Global => "global", SkillInstallTargetLayer::User => "user" },
            })),
            session_id: None,
        })
        .await;
        return Err("当前账号没有安装 Skill 的权限。".into());
    }

    let target_root = resolve_skill_install_target_dir(
        state.agent_runtime.as_ref(),
        &request.target_layer,
        request.target_scope.as_deref(),
    )?;
    tokio::fs::create_dir_all(&target_root).await?;

    let temp_dir = tempfile::tempdir()?;
    let archive_bytes = match request.source_type {
        SkillInstallSourceType::Skillhub => {
            let client = build_skillhub_http_client()?;
            fetch_skillhub_archive(&client, &request).await?
        }
        SkillInstallSourceType::DingtalkAttachment => {
            fetch_dingtalk_attachment_archive(state, &request).await?
        }
    };
    let skill_name = unpack_skill_archive(&archive_bytes, temp_dir.path(), &request.package).await?;
    let source_dir = temp_dir.path().join(&skill_name);
    let destination_dir = target_root.join(&skill_name);
    if destination_dir.exists() {
        return Err(format!("Skill {} 已存在，暂不支持覆盖安装", skill_name).into());
    }
    tokio::fs::rename(&source_dir, &destination_dir).await?;
    install_python_skill_dependencies(&destination_dir).await?;
    ensure_generated_skill_toml_if_missing(&destination_dir).await?;
    let _ = refresh_runtime_skills(state.agent_runtime.as_ref()).await?;

    let target_layer = match request.target_layer {
        SkillInstallTargetLayer::Global => "global",
        SkillInstallTargetLayer::User => "user",
    }
    .to_string();
    info!(
        action = "skill_install",
        channel = actor.channel,
        sender_user_id = actor.sender_user_id.as_deref().unwrap_or(""),
        sender_staff_id = actor.sender_staff_id.as_deref().unwrap_or(""),
        sender_corp_id = actor.sender_corp_id.as_deref().unwrap_or(""),
        package = request.package.as_str(),
        skill_name = skill_name.as_str(),
        target_layer = target_layer.as_str(),
        target_scope = request.target_scope.as_deref().unwrap_or(""),
        result = "success",
        "Installed skill successfully"
    );
    let _ = log_audit_event(AuditEvent {
        timestamp: chrono::Utc::now().timestamp() as u64,
        level: AuditLevel::Info,
        category: AuditCategory::Tool,
        actor: actor.sender_user_id.clone().or(actor.sender_staff_id.clone()),
        action: "skill_install_succeeded".to_string(),
        target: Some(skill_name.clone()),
        details: Some(serde_json::json!({
            "package": request.package,
            "channel": actor.channel,
            "source_type": source_type_label,
            "target_layer": target_layer,
            "target_scope": request.target_scope,
        })),
        session_id: None,
    })
    .await;

    Ok(SkillInstallResponse {
        skill_name,
        source_type: source_type_label.to_string(),
        package: request.package,
        version: request.version,
        target_layer,
        target_scope: request.target_scope,
    })
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

    async fn load_scoped_runtime_dir(
        base_dir: &FsPath,
        dir_name: &str,
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
            let catalog = load_agent_catalog_from_root(&path, true).await?;
            if !catalog.is_empty() {
                layered_agents.register_scoped_catalog(scope.to_string(), catalog);
            }
        }

        Ok(())
    }

    load_scoped_runtime_dir(&base_dir, "tenants", &mut layered_agents).await?;
    load_scoped_runtime_dir(&base_dir, "enterprises", &mut layered_agents).await?;
    load_scoped_runtime_dir(&base_dir, "departments", &mut layered_agents).await?;
    load_scoped_runtime_dir(&base_dir, "roles", &mut layered_agents).await?;
    load_scoped_runtime_dir(&base_dir, "users", &mut layered_agents).await?;

    let memory = LayeredMemoryStore::new(base_dir.join("workspace"));
    memory.init_workspace().await?;
    let layered_skills = load_runtime_skills(&base_dir).await?;

    Ok(WebAgentRuntime {
        runtime_root: base_dir,
        agent_manager: Arc::new(agent_manager),
        agents: Arc::new(layered_agents),
        memory_store: Arc::new(memory),
        skills: Arc::new(RwLock::new(layered_skills)),
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

    if let Some(turn_state) = state
        .hub
        .session_runtime()
        .turn_state(&session_key.as_str())
        .await
    {
        if let Some(summary) = turn_state.compacted_summary.as_deref() {
            sections.push(format!(
                "--- Compacted Turn Summary ---\n{}\n\ncovered_events: {}",
                summary,
                turn_state.pruned_event_count
            ));
        }
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

async fn load_or_init_session_state_for_update(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
) -> Option<(Arc<AgentScope>, SessionState, Option<CatalogAgentEntry>)> {
    let resolved_entry = resolve_agent_entry_for_session(state, session_key, Some(agent_id)).await;
    let scope = resolved_entry
        .as_ref()
        .and_then(agent_scope_from_entry)
        .or_else(|| agent_scope_for(state.as_ref(), agent_id))?;
    let session_state = match scope.load_session_state(&session_key.as_str()).await {
        Ok(Some(existing)) => existing,
        _ => SessionState::new(session_key.as_str()),
    };
    Some((scope, session_state, resolved_entry))
}

async fn save_session_state_with_scope(
    scope: &AgentScope,
    session_key: &SessionKey,
    session_state: &SessionState,
) {
    if let Err(error) = scope
        .save_session_state(&session_key.as_str(), session_state)
        .await
    {
        warn!(
            "Failed to persist session state for {}: {}",
            session_key, error
        );
    }
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
    let Some((scope, mut session_state, resolved_entry)) =
        load_or_init_session_state_for_update(state, session_key, agent_id).await
    else {
        return;
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

    save_session_state_with_scope(scope.as_ref(), session_key, &session_state).await;
}

async fn append_pending_dingtalk_attachments(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
    attachments: &[PendingDingTalkAttachment],
) {
    if attachments.is_empty() {
        return;
    }
    let Some((scope, mut session_state, _)) =
        load_or_init_session_state_for_update(state, session_key, agent_id).await
    else {
        return;
    };
    let mut existing = read_pending_dingtalk_attachments(&session_state.metadata);
    existing.extend_from_slice(attachments);
    if let Ok(serialized) = serde_json::to_string(&existing) {
        session_state.metadata.insert(
            DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY.to_string(),
            serialized,
        );
        save_session_state_with_scope(scope.as_ref(), session_key, &session_state).await;
    }
}

async fn set_last_audio_transcript(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
    transcript: &str,
) {
    let transcript = transcript.trim();
    if transcript.is_empty() {
        return;
    }
    let Some((scope, mut session_state, _)) =
        load_or_init_session_state_for_update(state, session_key, agent_id).await
    else {
        return;
    };
    session_state.metadata.insert(
        DINGTALK_LAST_AUDIO_TRANSCRIPT_KEY.to_string(),
        transcript.to_string(),
    );
    save_session_state_with_scope(scope.as_ref(), session_key, &session_state).await;
}

fn read_pending_dingtalk_attachments(
    metadata: &HashMap<String, String>,
) -> Vec<PendingDingTalkAttachment> {
    metadata
        .get(DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY)
        .and_then(|value| serde_json::from_str::<Vec<PendingDingTalkAttachment>>(value).ok())
        .unwrap_or_default()
}

async fn clear_pending_dingtalk_attachments(
    state: &Arc<WebState>,
    session_key: &SessionKey,
    agent_id: &str,
) {
    let Some((scope, mut session_state, _)) =
        load_or_init_session_state_for_update(state, session_key, agent_id).await
    else {
        return;
    };
    if session_state
        .metadata
        .remove(DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY)
        .is_some()
    {
        save_session_state_with_scope(scope.as_ref(), session_key, &session_state).await;
    }
}

fn build_pending_attachment_prefix(attachments: &[PendingDingTalkAttachment]) -> Option<String> {
    if attachments.is_empty() {
        return None;
    }
    let lines = attachments
        .iter()
        .map(|attachment| format!("- {}", attachment.summary))
        .collect::<Vec<_>>();
    Some(format!(
        "用户刚刚发送了以下附件上下文：\n{}\n\n用户本条补充说明：",
        lines.join("\n")
    ))
}

fn normalize_pending_attachment_package_name(file_name: Option<&str>) -> Option<String> {
    let file_name = file_name?.trim();
    if file_name.is_empty() {
        return None;
    }
    let lower = file_name.to_ascii_lowercase();
    let stem = if let Some(stripped) = lower.strip_suffix(".tar.gz") {
        &file_name[..stripped.len()]
    } else if let Some(stripped) = lower.strip_suffix(".tgz") {
        &file_name[..stripped.len()]
    } else if let Some(stripped) = lower.strip_suffix(".zip") {
        &file_name[..stripped.len()]
    } else {
        file_name
    };
    let stem = stem.trim().trim_matches('.').trim_matches('_').trim_matches('-');
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

fn is_pending_skill_archive_attachment(attachment: &PendingDingTalkAttachment) -> bool {
    if attachment.kind != "file" || attachment.download_code.is_none() {
        return false;
    }
    attachment
        .file_name
        .as_deref()
        .map(|name| {
            let lower = name.trim().to_ascii_lowercase();
            lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
        })
        .unwrap_or(false)
}

fn pending_attachment_matches_install_query(
    text: &str,
    attachment: &PendingDingTalkAttachment,
) -> bool {
    let query = text.trim();
    if query.is_empty() {
        return false;
    }

    let query_lower = query.to_ascii_lowercase();
    let package = normalize_pending_attachment_package_name(attachment.file_name.as_deref())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let file_name = attachment
        .file_name
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();

    (!file_name.is_empty() && query_lower == file_name)
        || (!package.is_empty() && query_lower == package)
        || (!file_name.is_empty() && query_lower.contains(&file_name))
        || (!package.is_empty() && query_lower.contains(&package))
}

fn build_pending_attachment_install_request(
    text: &str,
    attachments: &[PendingDingTalkAttachment],
) -> Option<SkillInstallRequest> {
    let attachment = if looks_like_dingtalk_skill_install_intent(text) {
        attachments
            .iter()
            .rev()
            .find(|attachment| is_pending_skill_archive_attachment(attachment))?
    } else {
        attachments
            .iter()
            .rev()
            .find(|attachment| {
                is_pending_skill_archive_attachment(attachment)
                    && pending_attachment_matches_install_query(text, attachment)
            })?
    };
    let package = normalize_pending_attachment_package_name(attachment.file_name.as_deref())
        .unwrap_or_else(|| "uploaded-skill".to_string());
    Some(SkillInstallRequest {
        source_type: SkillInstallSourceType::DingtalkAttachment,
        package,
        version: None,
        download_url: String::new(),
        target_layer: SkillInstallTargetLayer::Global,
        target_scope: None,
        attachment_download_code: attachment.download_code.clone(),
        attachment_file_name: attachment.file_name.clone(),
    })
}

fn pending_attachment_from_inbound_attachment(
    attachment: &DingTalkInboundAttachment,
) -> PendingDingTalkAttachment {
    let summary = match attachment.kind.as_str() {
        "image" => {
            let base = attachment
                .file_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("图片（{}）", value))
                .unwrap_or_else(|| "图片".to_string());
            match attachment.caption.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                Some(caption) => format!("{}：{}", base, caption),
                None => base,
            }
        }
        "file" => attachment
            .file_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("文件（{}）", value))
            .unwrap_or_else(|| "文件".to_string()),
        "audio" => attachment
            .file_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("语音（{}）", value))
            .unwrap_or_else(|| "语音消息".to_string()),
        _ => attachment.kind.clone(),
    };
    PendingDingTalkAttachment {
        kind: attachment.kind.clone(),
        summary,
        file_name: attachment
            .file_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        download_code: attachment
            .download_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
}

fn summarize_dingtalk_inbound_user_text(
    inbound: &DingTalkInboundMessage,
    consumed_pending_attachments: &[PendingDingTalkAttachment],
) -> String {
    match &inbound.message.content {
        MessageContent::Text(text) => {
            let text = text.trim();
            if consumed_pending_attachments.is_empty() {
                return text.to_string();
            }
            if let Some(install_request) = build_pending_attachment_install_request(
                text,
                consumed_pending_attachments,
            ) {
                return format!(
                    "{}（基于刚收到的附件：{}）",
                    text,
                    install_request.package
                );
            }
            format!(
                "{}（结合附件上下文：{}）",
                text,
                consumed_pending_attachments
                    .iter()
                    .map(|attachment| attachment.summary.as_str())
                    .collect::<Vec<_>>()
                    .join("，")
            )
        }
        MessageContent::Image { caption, .. } => caption
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("用户发送了一张图片：{}", value))
            .or_else(|| {
                inbound
                    .attachments
                    .iter()
                    .find(|attachment| attachment.kind == "image")
                    .map(pending_attachment_from_inbound_attachment)
                    .map(|attachment| attachment.summary)
            })
            .unwrap_or_else(|| "用户发送了一张图片".to_string()),
        MessageContent::Audio { .. } => inbound
            .attachments
            .iter()
            .find(|attachment| attachment.kind == "audio")
            .and_then(|attachment| attachment.recognition.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                inbound
                    .attachments
                    .iter()
                    .find(|attachment| attachment.kind == "audio")
                    .map(pending_attachment_from_inbound_attachment)
                    .map(|attachment| attachment.summary)
            })
            .unwrap_or_else(|| "用户发送了一条语音消息".to_string()),
        MessageContent::Structured(data) => {
            if data.get("kind").and_then(Value::as_str) == Some("dingtalk_file") {
                inbound
                    .attachments
                    .iter()
                    .find(|attachment| attachment.kind == "file")
                    .map(pending_attachment_from_inbound_attachment)
                    .map(|attachment| attachment.summary)
                    .or_else(|| {
                        data.get("file_name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(|value| format!("文件（{}）", value))
                    })
                    .unwrap_or_else(|| "文件".to_string())
            } else {
                serde_json::to_string(data)
                    .unwrap_or_else(|_| data.to_string())
                    .trim()
                    .to_string()
            }
        }
    }
}

fn reply_route_from_inbound(inbound: &DingTalkInboundMessage) -> DingTalkReplyRoute {
    DingTalkReplyRoute {
        conversation_id: inbound.conversation_id.clone(),
        source_message_id: inbound.message_id.clone(),
        conversation_type: inbound.conversation_type.clone(),
        sender_user_id: inbound.sender_user_id.clone(),
        sender_staff_id: inbound.sender_staff_id.clone(),
        session_webhook: inbound.session_webhook.clone(),
        session_webhook_expired_time: inbound.session_webhook_expired_time,
        robot_code: inbound.robot_code.clone(),
    }
}

async fn try_create_early_dingtalk_reply_handle(
    state: &Arc<WebState>,
    inbound: &DingTalkInboundMessage,
) -> Result<Option<DingTalkReplyHandle>, Box<dyn std::error::Error + Send + Sync>> {
    if !should_attach_dingtalk_processing_ack_now(inbound) {
        return Ok(None);
    }

    let Some(channel) = state.dingtalk_channel.as_ref() else {
        return Ok(None);
    };

    let route = reply_route_from_inbound(inbound);
    Ok(Some(create_dingtalk_reply_handle(channel, &route).await?))
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
        runtime_mailbox: None,
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
        .read()
        .await
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
    let mailbox_snapshots = state.hub.session_runtime().mailbox_snapshots().await;
    let mailbox_map: HashMap<_, _> = mailbox_snapshots
        .into_iter()
        .map(|snapshot| (snapshot.session_key.clone(), snapshot))
        .collect();

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

            let mut detail = session_state_to_detail(&session_state);
            detail.runtime_mailbox = mailbox_map.get(&detail.session_id).cloned();
            let should_replace = sessions
                .get(&detail.session_id)
                .map(|existing: &SessionRuntimeDetail| detail.last_active > existing.last_active)
                .unwrap_or(true);
            if should_replace {
                sessions.insert(detail.session_id.clone(), detail);
            }
        }
    }

    for (session_key, mailbox) in mailbox_map {
        sessions.entry(session_key.clone()).or_insert_with(|| SessionRuntimeDetail {
            session_id: session_key,
            agent_id: None,
            conversation_id: None,
            sender_user_id: None,
            sender_staff_id: None,
            last_task_id: None,
            message_count: 0,
            created_at: String::new(),
            last_active: String::new(),
            namespace: None,
            collaboration_workspace: None,
            memory_context_chain: vec![],
            visibility_chain: vec![],
            metadata: HashMap::new(),
            runtime_mailbox: Some(mailbox),
        });
    }

    let mut values: Vec<_> = sessions.into_values().collect();
    values.sort_by(|left, right| right.last_active.cmp(&left.last_active));
    values
}

async fn read_session_messages(
    state: &Arc<WebState>,
    session_id: &str,
) -> Result<Vec<SessionMessageRecord>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(transcript) = state.hub.session_runtime().transcript(session_id).await {
        let projected = project_transcript_messages(&transcript);
        if !projected.is_empty() {
            return Ok(projected);
        }
    }

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

fn project_transcript_messages(
    transcript: &crate::session_runtime::SessionTranscript,
) -> Vec<SessionMessageRecord> {
    let mut current_user: Option<(String, String)> = None;
    let mut assistant_events: Vec<String> = Vec::new();
    let mut records = Vec::new();

    for event in &transcript.events {
        match event.kind {
            TranscriptEventKind::UserMessage => {
                current_user = Some((event.created_at.to_rfc3339(), event.content.clone()));
                assistant_events.clear();
            }
            TranscriptEventKind::AssistantFinal => {
                if let Some((timestamp, user_message)) = current_user.take() {
                    let assistant_message = if assistant_events.is_empty() {
                        event.content.clone()
                    } else {
                        format!("{}\n\n{}", assistant_events.join("\n"), event.content)
                    };
                    records.push(SessionMessageRecord {
                        timestamp,
                        user_message,
                        assistant_message,
                    });
                }
                assistant_events.clear();
            }
            TranscriptEventKind::AssistantStep => {
                assistant_events.push(format!("[assistant_step] {}", event.content));
            }
            TranscriptEventKind::ToolCallPlanned => {
                assistant_events.push(format!("[tool_call_planned] {}", event.content));
            }
            TranscriptEventKind::ToolCallDispatched => {
                assistant_events.push(format!("[tool_call_dispatched] {}", event.content));
            }
            TranscriptEventKind::ApprovalRequested => {
                assistant_events.push(format!("[approval_requested] {}", event.content));
            }
            TranscriptEventKind::ToolResultObserved => {
                assistant_events.push(format!("[tool_result_observed] {}", event.content));
            }
            TranscriptEventKind::ApprovalApproved => {
                assistant_events.push(format!("[approval_approved] {}", event.content));
            }
            TranscriptEventKind::ApprovalRejected => {
                assistant_events.push(format!("[approval_rejected] {}", event.content));
            }
            TranscriptEventKind::TurnResumed => {
                assistant_events.push(format!("[turn_resumed] {}", event.content));
            }
            TranscriptEventKind::PlannerRetry => {
                assistant_events.push(format!("[planner_retry] {}", event.content));
            }
            TranscriptEventKind::TurnCompacted => {
                assistant_events.push(format!("[turn_compacted] {}", event.content));
            }
            TranscriptEventKind::TurnFailed => {
                assistant_events.push(format!("[turn_failed] {}", event.content));
            }
            TranscriptEventKind::TurnCancelled => {
                assistant_events.push(format!("[turn_cancelled] {}", event.content));
            }
        }
    }

    records
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
    create_router_with_health_config(state, &HealthConfig::default())
}

/// 使用指定健康检查路径创建 Web 路由
pub fn create_router_with_health_path(state: WebState, health_path: &str) -> Router {
    let health_config = HealthConfig {
        enabled: true,
        path: health_path.to_string(),
        ..HealthConfig::default()
    };
    create_router_with_health_config(state, &health_config)
}

/// 使用健康检查配置创建 Web 路由
pub fn create_router_with_health_config(state: WebState, health_config: &HealthConfig) -> Router {
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
        .route("/api/runtime/diagnostics", get(get_runtime_diagnostics))
        .route("/api/approvals", get(list_approvals))
        .route("/api/approvals/:request_id", get(get_approval))
        .route("/api/approvals/:request_id/approve", post(approve_approval))
        .route("/api/approvals/:request_id/reject", post(reject_approval))
        .route("/api/node-auth/token", post(issue_node_token))
        .route("/api/account/pairing/start", post(start_account_pairing))
        .route("/api/account/pairing/cancel", post(cancel_account_pairing))
        .route("/api/account/status/:node_id", get(get_account_status))
        .route(
            "/api/account/binding/:node_id",
            axum::routing::delete(delete_account_binding),
        )
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/agents", get(list_runtime_agents))
        .route("/api/v1/agents/:agent_id", get(get_runtime_agent))
        .route("/api/v1/skills", get(list_runtime_skills))
        .route("/api/v1/skills/install", post(install_runtime_skill))
        .route("/api/v1/skills/refresh", post(refresh_runtime_skill))
        .route("/api/v1/skills/:skill_name", get(get_runtime_skill))
        .route("/api/v1/sessions", get(list_runtime_sessions))
        .route("/api/v1/sessions/:session_id", get(get_runtime_session))
        .route(
            "/api/v1/sessions/:session_id/messages",
            get(get_runtime_session_messages),
        );

    if health_config.enabled {
        router = router.route(&health_config.path, get(health_check));
    }

    let router: Router = router.with_state(shared_state);

    // 添加 CORS、HTTP tracing 与 metrics
    router
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            metrics_state,
            track_api_metrics,
        ))
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
            if let Err(error) = handle_dingtalk_inbound(&state, inbound.clone()).await {
                error!("Failed to handle DingTalk inbound message: {}", error);
                if let Err(reply_error) = reply_dingtalk_error(&state, &inbound, &error.to_string()).await {
                    error!(
                        "Failed to reply DingTalk inbound error for conversation {}: {}",
                        inbound.conversation_id,
                        reply_error
                    );
                }
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "status": "ok"
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

/// 处理 DingTalk 入站消息。
pub async fn handle_dingtalk_inbound(
    state: &Arc<WebState>,
    inbound: DingTalkInboundMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if try_handle_dingtalk_pairing_command(state, &inbound).await? {
        return Ok(());
    }

    let early_reply_handle = try_create_early_dingtalk_reply_handle(state, &inbound).await?;
    submit_dingtalk_task(state, inbound, early_reply_handle).await
}

/// 将 DingTalk 入站消息转换为 Hub 任务并提交执行
async fn build_dingtalk_stt_client(
    state: &Arc<WebState>,
) -> Result<SttClient, Box<dyn std::error::Error + Send + Sync>> {
    let llm = &state.app_config.llm;
    let api_key = llm.api_key.trim();
    if api_key.is_empty() {
        return Err("LLM API key is required for DingTalk STT".into());
    }
    let mut config = SttConfig::new(api_key.to_string());
    let base_url = llm.base_url.trim();
    if !base_url.is_empty() {
        config = config.with_api_base(base_url.to_string());
    }
    Ok(SttClient::new(config))
}

async fn normalize_dingtalk_inbound_message(
    state: &Arc<WebState>,
    inbound: &DingTalkInboundMessage,
    session_key: &SessionKey,
) -> Result<NormalizedDingTalkInbound, Box<dyn std::error::Error + Send + Sync>> {
    let agent_id = resolve_agent_id_for_session(state, session_key).await;
    let pending_attachments = load_session_state_for_session(state, session_key)
        .await
        .map(|session_state| read_pending_dingtalk_attachments(&session_state.metadata))
        .unwrap_or_default();

    let base_text = match &inbound.message.content {
        MessageContent::Text(text) => Some(text.trim().to_string()),
        MessageContent::Audio { .. } => {
            if let Some(transcript) = inbound
                .attachments
                .iter()
                .find(|attachment| attachment.kind == "audio")
                .and_then(|attachment| attachment.recognition.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
            {
                set_last_audio_transcript(state, session_key, &agent_id, &transcript).await;
                Some(transcript)
            } else {
                let channel = state
                    .dingtalk_channel
                    .as_ref()
                    .ok_or("DingTalk channel is unavailable")?;
                let bytes = channel
                    .download_inbound_message_media(inbound)
                    .await?
                    .ok_or("DingTalk audio download metadata is missing")?;
                let client = build_dingtalk_stt_client(state).await?;
                let result = client.transcribe(&bytes, "dingtalk-audio.mp3").await?;
                let transcript = result.text.trim().to_string();
                set_last_audio_transcript(state, session_key, &agent_id, &transcript).await;
                Some(transcript)
            }
        }
        MessageContent::Image { caption, .. } => caption
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("用户发送了一张图片：{}", value)),
        MessageContent::Structured(data) => {
            if data.get("kind").and_then(Value::as_str) == Some("dingtalk_file") {
                None
            } else {
                Some(
                    serde_json::to_string(data)
                        .unwrap_or_else(|_| data.to_string())
                        .trim()
                        .to_string(),
                )
            }
        }
    };

    let text = match base_text {
        Some(text) if !text.trim().is_empty() => text.trim().to_string(),
        _ => {
            let pending_attachments = inbound
                .attachments
                .iter()
                .map(pending_attachment_from_inbound_attachment)
                .collect::<Vec<_>>();
            return if pending_attachments.is_empty() {
                Ok(NormalizedDingTalkInbound::WaitForFollowUp {
                    reply_text: DINGTALK_ATTACHMENT_WAITING_REPLY_TEXT.to_string(),
                })
            } else {
                append_pending_dingtalk_attachments(
                    state,
                    session_key,
                    &agent_id,
                    &pending_attachments,
                )
                .await;
                Ok(NormalizedDingTalkInbound::WaitForFollowUp {
                    reply_text: DINGTALK_ATTACHMENT_WAITING_REPLY_TEXT.to_string(),
                })
            };
        }
    };

    let install_request = build_pending_attachment_install_request(&text, &pending_attachments);
    if !pending_attachments.is_empty() {
        clear_pending_dingtalk_attachments(state, session_key, &agent_id).await;
    }
    let merged = if let Some(request) = &install_request {
        format!(
            "帮我安装这个技能\n\n附件技能包：{}",
            request
                .attachment_file_name
                .as_deref()
                .unwrap_or(request.package.as_str())
        )
    } else if let Some(prefix) = build_pending_attachment_prefix(&pending_attachments) {
        format!("{}\n{}", prefix, text)
    } else {
        text
    };
    Ok(NormalizedDingTalkInbound::ContinueAsText {
        text: merged,
        consumed_pending_attachments: pending_attachments,
    })
}

async fn submit_dingtalk_task(
    state: &Arc<WebState>,
    inbound: DingTalkInboundMessage,
    early_reply_handle: Option<DingTalkReplyHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session_key = build_dingtalk_session_key(
        &inbound.session.channel_user_id,
        inbound.sender_user_id.as_deref(),
        inbound.sender_staff_id.as_deref(),
        inbound.sender_corp_id.as_deref(),
    );
    let normalized = normalize_dingtalk_inbound_message(state, &inbound, &session_key).await?;
    if let NormalizedDingTalkInbound::ContinueAsText {
        text,
        consumed_pending_attachments,
    } = normalized
    {
        let session_key_string = session_key.as_str().to_string();
        let state = Arc::clone(state);
        return state
            .hub
            .session_runtime()
            .run_serialized(&session_key_string, async move {
                process_dingtalk_task_serialized(
                    &state,
                    inbound,
                    session_key,
                    early_reply_handle,
                    text,
                    consumed_pending_attachments,
                )
                .await
            })
            .await;
    }

    if let NormalizedDingTalkInbound::WaitForFollowUp { reply_text } = normalized {
        let session_key_string = session_key.as_str().to_string();
        let state = Arc::clone(state);
        return state
            .hub
            .session_runtime()
            .run_serialized(&session_key_string, async move {
                let agent_id = resolve_agent_id_for_session(&state, &session_key).await;
                let route = reply_route_from_inbound(&inbound);
                let user_text = summarize_dingtalk_inbound_user_text(&inbound, &[]);
                let turn_id = state
                    .hub
                    .session_runtime()
                    .start_turn(&session_key.as_str(), user_text.clone())
                    .await;
                state
                    .hub
                    .session_runtime()
                    .append_transcript_event(
                        &session_key.as_str(),
                        TranscriptEventKind::AssistantFinal,
                        reply_text.clone(),
                    )
                    .await;
                state
                    .hub
                    .session_runtime()
                    .complete_turn(&session_key.as_str(), None)
                    .await;
                persist_direct_reply_memory(&state, &session_key, &agent_id, &user_text, &reply_text)
                    .await;
                persist_session_state(
                    &state,
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
                let _ = turn_id;
                if let Some(channel) = state.dingtalk_channel.as_ref() {
                    send_or_finalize_dingtalk_reply(
                        channel,
                        &route,
                        &reply_text,
                        early_reply_handle,
                        None,
                    )
                    .await?;
                }
                Ok(())
            })
            .await;
    }

    Ok(())
}

async fn process_dingtalk_task_serialized(
    state: &Arc<WebState>,
    inbound: DingTalkInboundMessage,
    session_key: SessionKey,
    early_reply_handle: Option<DingTalkReplyHandle>,
    text: String,
    consumed_pending_attachments: Vec<PendingDingTalkAttachment>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = text.trim().to_string();
    let agent_id = resolve_agent_id_for_session(state, &session_key).await;
    let route = reply_route_from_inbound(&inbound);
    let user_text = summarize_dingtalk_inbound_user_text(&inbound, &consumed_pending_attachments);
    let turn_id = state
        .hub
        .session_runtime()
        .start_turn(&session_key.as_str(), user_text.clone())
        .await;
    let step = plan_next_dingtalk_step(state, &text, &agent_id, &session_key).await?;
    state.metrics_collector.inc_loop_steps("initial_plan");
    state
        .hub
        .session_runtime()
        .append_transcript_event(
            &session_key.as_str(),
            TranscriptEventKind::AssistantStep,
            format!("{:?}", step),
        )
        .await;
    let turn_state = state
        .hub
        .session_runtime()
        .increment_step_count(&session_key.as_str())
        .await
        .ok_or_else(|| "session turn is missing before step execution".to_string())?;
    if turn_state.step_count > turn_state.max_steps {
        let reply_text = "当前这轮操作步骤过多，我先到这里，并基于已有结果收口。".to_string();
        state
            .hub
            .session_runtime()
            .append_transcript_event(
                &session_key.as_str(),
                TranscriptEventKind::AssistantFinal,
                reply_text.clone(),
            )
            .await;
        state
            .hub
            .session_runtime()
            .complete_turn(&session_key.as_str(), None)
            .await;
        if let Some(channel) = state.dingtalk_channel.as_ref() {
            send_dingtalk_reply(channel, &route, &reply_text).await?;
        }
        return Ok(());
    }

    let immediate_reply_handle = if should_send_dingtalk_immediate_ack(&step) {
        early_reply_handle.clone()
    } else {
        None
    };

    match step {
        PlannedTurnStep::Finalize { text: reply_text } => {
            let enforce_min_ack_display = true;
            state
                .hub
                .session_runtime()
                .append_transcript_event(
                    &session_key.as_str(),
                    TranscriptEventKind::AssistantFinal,
                    reply_text.clone(),
                )
                .await;
            state
                .hub
                .session_runtime()
                .complete_turn(&session_key.as_str(), None)
                .await;
            persist_direct_reply_memory(state, &session_key, &agent_id, &user_text, &reply_text).await;
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
                if enforce_min_ack_display {
                    enforce_dingtalk_ack_min_display(
                        immediate_reply_handle.as_ref(),
                        Duration::from_millis(DINGTALK_DIRECT_REPLY_MIN_ACK_DISPLAY_MILLIS),
                    )
                    .await;
                }
                send_or_finalize_dingtalk_reply(
                    channel,
                    &route,
                    &reply_text,
                    immediate_reply_handle,
                    None,
                )
                .await?;
            } else {
                warn!("Skip DingTalk direct reply because channel is unavailable");
            }
            info!(
                "Replied DingTalk message directly for session {} via agent {}",
                session_key, agent_id
            );
            Ok(())
        }
        PlannedTurnStep::ExecuteSkill { .. }
        | PlannedTurnStep::ListInstalledSkills
        | PlannedTurnStep::QuerySkill { .. }
        | PlannedTurnStep::InstallSkill { .. } => {
            execute_local_turn_step(
                state,
                &route,
                &session_key,
                &agent_id,
                &text,
                &step,
                &consumed_pending_attachments,
                inbound.sender_user_id.as_deref(),
                inbound.sender_staff_id.as_deref(),
                inbound.sender_corp_id.as_deref(),
                &inbound.conversation_id,
                immediate_reply_handle,
            )
            .await?;
            if let PlannedTurnStep::ExecuteSkill { skill_name, .. } = &step {
                info!(
                    "Executed local skill {} for session {} via agent {}",
                    skill_name, session_key, agent_id
                );
            }
            Ok(())
        }
        PlannedTurnStep::SubmitTask {
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

            let tool_call_id = state
                .hub
                .session_runtime()
                .begin_tool_call(&session_key.as_str(), "hub_task")
                .await
                .ok_or_else(|| "session turn is missing before task submission".to_string())?;
            state
                .hub
                .session_runtime()
                .append_transcript_event(
                    &session_key.as_str(),
                    TranscriptEventKind::ToolCallPlanned,
                    format!("{}:{}", tool_call_id, "hub_task"),
                )
                .await;
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
            state
                .hub
                .session_runtime()
                .bind_task_to_turn(
                    task_id.clone(),
                    TaskContinuationBinding {
                        session_key: session_key.as_str().to_string(),
                        turn_id: turn_id.clone(),
                        tool_call_id,
                        agent_id: agent_id.clone(),
                        route: route.clone(),
                    },
                )
                .await;

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

            if let Some(handle) = early_reply_handle {
                let mut handles = state.dingtalk_reply_handles.write().await;
                handles.insert(task_id.clone(), handle);
            } else if let Some(channel) = state.dingtalk_channel.as_ref() {
                let handle = create_dingtalk_reply_handle(channel, &route).await?;
                let mut handles = state.dingtalk_reply_handles.write().await;
                handles.insert(task_id.clone(), handle);
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

async fn execute_local_turn_step(
    state: &Arc<WebState>,
    route: &DingTalkReplyRoute,
    session_key: &SessionKey,
    agent_id: &str,
    user_text: &str,
    step: &PlannedTurnStep,
    consumed_pending_attachments: &[PendingDingTalkAttachment],
    sender_user_id: Option<&str>,
    sender_staff_id: Option<&str>,
    sender_corp_id: Option<&str>,
    conversation_id: &str,
    reply_handle: Option<DingTalkReplyHandle>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let reply_text = match step {
        PlannedTurnStep::ExecuteSkill { skill_name, input } => {
            execute_local_skill(state, session_key, skill_name, input).await?
        }
        PlannedTurnStep::ListInstalledSkills => {
            let mut skills: Vec<_> = state
                .agent_runtime
                .skills
                .read()
                .await
                .list_all_entries()
                .into_iter()
                .map(skill_to_summary)
                .collect();
            skills.sort_by(|left, right| left.name.cmp(&right.name));
            build_installed_skills_reply(&skills)
        }
        PlannedTurnStep::QuerySkill { skill_name } => {
            let skill_entry = state.agent_runtime.skills.read().await.get_any_entry(skill_name);
            if let Some(entry) = skill_entry {
                let detail = skill_to_detail(entry);
                format!(
                    "Skill {}：{}。当前版本：{}。{}",
                    detail.name,
                    detail.description,
                    detail.version,
                    if detail.enabled { "当前已启用" } else { "当前未启用" }
                )
            } else {
                format!("没有找到名为 {} 的 Skill。", skill_name)
            }
        }
        PlannedTurnStep::InstallSkill { query } => {
            let intent = resolve_dingtalk_skill_install_intent(
                state.as_ref(),
                query,
                &consumed_pending_attachments,
            )
            .await?;
            let actor = SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: sender_user_id.map(str::to_string),
                sender_staff_id: sender_staff_id.map(str::to_string),
                sender_corp_id: sender_corp_id.map(str::to_string),
            };
            match intent {
                Some(DingtalkSkillInstallIntent::ExplicitCommand(request)) => {
                    let requested_name = request.package.clone();
                    let failed_attachment = matches!(request.source_type, SkillInstallSourceType::DingtalkAttachment)
                        .then(|| PendingDingTalkAttachment {
                            kind: "file".to_string(),
                            summary: request
                                .attachment_file_name
                                .as_deref()
                                .map(|value| format!("文件（{}）", value.trim()))
                                .unwrap_or_else(|| format!("文件（{}）", requested_name)),
                            file_name: request.attachment_file_name.clone(),
                            download_code: request.attachment_download_code.clone(),
                        });
                    match install_skill_from_request(state, actor, request).await {
                        Ok(result) => {
                            let trigger_hint = build_skill_install_trigger_hint(state, &result).await;
                            format!("Skill {} 安装成功。{}", result.skill_name, trigger_hint)
                        }
                        Err(error) => {
                            if let Some(attachment) = failed_attachment.as_ref() {
                                append_pending_dingtalk_attachments(
                                    state,
                                    session_key,
                                    agent_id,
                                    std::slice::from_ref(attachment),
                                )
                                .await;
                            }
                            format!("Skill {} 安装失败：{}", requested_name, error)
                        }
                    }
                }
                Some(DingtalkSkillInstallIntent::NaturalLanguage(entry)) => {
                    let requested_name = entry.name.clone();
                    let request = SkillInstallRequest {
                        source_type: SkillInstallSourceType::Skillhub,
                        package: entry.slug.clone(),
                        version: entry.version,
                        download_url: build_skillhub_download_url(state.as_ref(), &entry.slug),
                        target_layer: SkillInstallTargetLayer::Global,
                        target_scope: None,
                        attachment_download_code: None,
                        attachment_file_name: None,
                    };
                    match install_skill_from_request(state, actor, request).await {
                        Ok(result) => {
                            let trigger_hint = build_skill_install_trigger_hint(state, &result).await;
                            format!("Skill {} 安装成功。{}", result.skill_name, trigger_hint)
                        }
                        Err(error) => format!("Skill {} 安装失败：{}", requested_name, error),
                    }
                }
                Some(DingtalkSkillInstallIntent::NaturalLanguageNoMatch(query)) => {
                    format!("没有在 SkillHub 中找到与“{}”匹配的 Skill。", query)
                }
                None => "我理解到你在说 Skill，但没有识别出明确的安装目标。请直接说出要安装的 Skill 名称。".to_string(),
            }
        }
        _ => return Err("unsupported local turn step".into()),
    };

    state
        .hub
        .session_runtime()
        .append_transcript_event(
            &session_key.as_str(),
            TranscriptEventKind::AssistantFinal,
            reply_text.clone(),
        )
        .await;
    state
        .hub
        .session_runtime()
        .complete_turn(&session_key.as_str(), None)
        .await;
    persist_direct_reply_memory(state, session_key, agent_id, user_text, &reply_text).await;
    persist_session_state(
        state,
        session_key,
        agent_id,
        conversation_id,
        sender_user_id,
        sender_staff_id,
        None,
        None,
        None,
    )
    .await;
    if let Some(channel) = state.dingtalk_channel.as_ref() {
        if should_enforce_min_ack_display(step) {
            enforce_dingtalk_ack_min_display(
                reply_handle.as_ref(),
                Duration::from_millis(DINGTALK_DIRECT_REPLY_MIN_ACK_DISPLAY_MILLIS),
            )
            .await;
        }
        send_or_finalize_dingtalk_reply(channel, route, &reply_text, reply_handle, None).await?;
    }
    Ok(reply_text)
}

async fn plan_next_dingtalk_step(
    state: &Arc<WebState>,
    text: &str,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<PlannedTurnStep, Box<dyn std::error::Error + Send + Sync>> {
    let decision = decide_dingtalk_action(state, &text, agent_id, session_key).await?;
    let _ = log_audit_event(AuditEvent {
        timestamp: chrono::Utc::now().timestamp() as u64,
        level: AuditLevel::Info,
        category: AuditCategory::Session,
        actor: None,
        action: "planner_decision_made".to_string(),
        target: Some(session_key.as_str().to_string()),
        details: Some(serde_json::json!({
            "agent_id": agent_id,
            "decision": match &decision {
                AgentDecision::DirectReply { .. } => "direct_reply",
                AgentDecision::ExecuteCommand { .. } => "execute_command",
                AgentDecision::ExecuteSkill { .. } => "execute_skill",
                AgentDecision::ListInstalledSkills => "list_installed_skills",
                AgentDecision::QuerySkill { .. } => "query_skill",
                AgentDecision::InstallSkill { .. } => "install_skill",
            },
        })),
        session_id: Some(session_key.as_str().to_string()),
    })
    .await;
    Ok(planned_step_from_agent_decision(decision))
}

fn planned_step_from_agent_decision(decision: AgentDecision) -> PlannedTurnStep {
    match decision {
        AgentDecision::DirectReply { text } => PlannedTurnStep::Finalize { text },
        AgentDecision::ExecuteCommand {
            command,
            workspace_path,
        } => PlannedTurnStep::SubmitTask {
            command,
            workspace_path,
        },
        AgentDecision::ExecuteSkill { skill_name, input } => {
            PlannedTurnStep::ExecuteSkill { skill_name, input }
        }
        AgentDecision::ListInstalledSkills => PlannedTurnStep::ListInstalledSkills,
        AgentDecision::QuerySkill { skill_name } => PlannedTurnStep::QuerySkill { skill_name },
        AgentDecision::InstallSkill { query } => PlannedTurnStep::InstallSkill { query },
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
            &text,
            agent_id,
            session_key,
            &context,
            &online_workspace_roots,
            default_workspace_root.as_deref(),
            &state
                .agent_runtime
                .skills
                .read()
                .await
                .list_names_for_scopes(&visibility_chain),
        ))
        .await?;

    parse_agent_decision(
        state,
        &response,
        &text,
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
    let trimmed = response.trim();

    if let Some(json_payload) = extract_first_json_object(trimmed) {
        if let Ok(decision) = serde_json::from_str::<AgentDecision>(&json_payload) {
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
                AgentDecision::DirectReply { .. } if should_force_dingtalk_command_planning(text) => {
                    if let Some(default_workspace_root) = default_workspace_root.as_deref() {
                        plan_dingtalk_command(
                            state,
                            &text,
                            default_workspace_root,
                            online_workspace_roots,
                            agent_id,
                            session_key,
                        )
                        .await
                    } else {
                        Ok(AgentDecision::DirectReply {
                            text: direct_reply_for_forced_planning_without_workspace(text)
                                .unwrap_or_else(|| response.trim().to_string()),
                        })
                    }
                }
                other => Ok(other),
            };
        }

        if let Some(default_workspace_root) = default_workspace_root.as_deref() {
            if let Ok(planned) = parse_planned_command(&json_payload, default_workspace_root) {
                return Ok(AgentDecision::ExecuteCommand {
                    command: planned.command,
                    workspace_path: planned.workspace_path,
                });
            }
        }
    }

    if !trimmed.is_empty() {
        if let Some(default_workspace_root) = default_workspace_root.as_deref() {
            if let Ok(planned) = parse_planned_command(trimmed, default_workspace_root) {
                return Ok(AgentDecision::ExecuteCommand {
                    command: planned.command,
                    workspace_path: planned.workspace_path,
                });
            }
        }

        if should_force_dingtalk_command_planning(text) {
            if let Some(default_workspace_root) = default_workspace_root.as_deref() {
                return plan_dingtalk_command(
                    state,
                    &text,
                    default_workspace_root,
                    online_workspace_roots,
                    agent_id,
                    session_key,
                )
                .await;
            }

            return Ok(AgentDecision::DirectReply {
                text: direct_reply_for_forced_planning_without_workspace(text)
                    .unwrap_or_else(|| trimmed.to_string()),
            });
        }

        if trimmed.starts_with('{') {
            return Ok(AgentDecision::DirectReply {
                text: "我没有正确理解你的意思。请直接告诉我你希望我怎么处理刚刚上传的附件，例如：帮我安装这个技能。".to_string(),
            });
        }

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
        &text,
        &default_workspace_root,
        &online_workspace_roots,
        agent_id,
        session_key,
    )
    .await
}

fn extract_first_json_object(input: &str) -> Option<String> {
    let start = input.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for (offset, ch) in input[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(input[start..end].to_string());
                }
            }
            _ => {}
        }
    }

    None
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
            &text,
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
                "你是 uHorse Hub 的 Agent 决策器。你必须根据用户输入与上下文，只输出一个 JSON 对象，不要输出 Markdown、解释或代码块。允许六种结构：1）直接回复：{{\"type\":\"direct_reply\",\"text\":\"...\"}}；2）需要继续规划命令：{{\"type\":\"execute_command\",\"command\": <uhorse_protocol::Command JSON>, \"workspace_path\": \"...\"}}；3）执行 Hub 本地技能：{{\"type\":\"execute_skill\",\"skill_name\":\"...\",\"input\":\"...\"}}；4）列出已安装技能：{{\"type\":\"list_installed_skills\"}}；5）查询某个技能：{{\"type\":\"query_skill\",\"skill_name\":\"...\"}}；6）安装技能：{{\"type\":\"install_skill\",\"query\":\"...\"}}。优先做正确意图理解，不要因为句子里出现“技能”“安装”等字样就误判。像“帮我列出已经安装的技能”必须返回 list_installed_skills；像“帮我安装 Browser Use 技能”才返回 install_skill；像“Browser Use 技能怎么用”优先返回 query_skill 或 direct_reply。只有确实需要 Node 执行文件、shell 或浏览器操作时才返回 execute_command。只有当请求明确适合本地技能时才返回 execute_skill。可用技能列表：{}。禁止生成 code/database/api 命令。browser 命令只允许访问安全的 http/https 公网页面，不允许 localhost、127.0.0.1、私网 IP、file:// 等本机或内网目标。用户只是要在宿主机打开网页时使用 open_system；只有需要继续读取网页内容、点击或抓取文本时才使用 navigate / wait_for / get_text / close。shell 命令请优先输出最简合法 JSON，例如 {{\"type\":\"shell\",\"command\":\"pwd\"}} 或 {{\"type\":\"shell\",\"command\":\"git\",\"args\":[\"status\"]}}。若返回 execute_command，workspace_path 必须填写目标 Node workspace 根路径；路径必须限制在该 workspace 内，不允许绝对路径越界，不允许使用 ..。若当前存在多个在线 workspace，workspace_path 必须显式填写，并且只能从提供的在线 workspace 列表中选择。下方的 Agent Workspace Context 和 Session Memory Context 仅供决策参考，不等于 Node 实际工作目录。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。",
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

fn should_force_dingtalk_command_planning(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let keywords = [
        "天气", "气温", "温度", "降雨", "下雨", "预报", "实时", "最新", "汇率", "股价",
        "新闻", "热搜", "航班", "机票", "列车", "火车票", "高铁", "路况", "地图", "导航",
        "搜一下", "查一下", "查询", "帮我查", "请查", "what's the weather", "weather",
        "temperature", "forecast", "real-time", "realtime", "live", "news", "stock price",
        "exchange rate", "flight", "train", "traffic", "map", "navigate",
    ];

    keywords
        .iter()
        .any(|keyword| trimmed.contains(keyword) || lower.contains(&keyword.to_ascii_lowercase()))
}

fn direct_reply_for_forced_planning_without_workspace(text: &str) -> Option<String> {
    if !should_force_dingtalk_command_planning(text) {
        return None;
    }

    Some(
        "当前 Node Desktop 不在线，暂时无法执行实时查询。请先启动 Node Desktop，启动后我就可以继续帮你查询。"
            .to_string(),
    )
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
        "你是 uHorse Hub 的任务规划器。你必须把用户的自然语言请求转换为单个 JSON 对象，且只能输出 JSON，不要输出 Markdown、解释或代码块。JSON 结构必须是 {\"command\": <uhorse_protocol::Command JSON>, \"workspace_path\": \"...\" }。优先生成 file 命令；只有文件命令无法完成时才生成 shell 或 browser 命令。禁止生成 code/database/api/skill 命令。workspace_path 必须填写目标 Node workspace 根路径。路径必须限制在 workspace_path 内，不允许绝对路径越界，不允许使用 ..。若当前存在多个在线 workspace，workspace_path 必须显式填写，并且只能从提供的在线 workspace 列表中选择。下方的 Agent Workspace Context 和 Session Memory Context 仅供参考，不等于 Node 实际工作目录。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。browser 命令只允许访问安全的 http/https 公网页面，不允许 localhost、127.0.0.1、私网 IP、file:// 等本机或内网目标；仅当用户要在宿主机打开网页时使用 open_system；读取网页内容或继续交互时使用 navigate / wait_for / get_text / close。shell 命令只允许只读、安全的本地仓库检查或目录查看，优先输出最简合法 JSON，例如 {{\"type\":\"shell\",\"command\":\"pwd\"}} 或 {{\"type\":\"shell\",\"command\":\"git\",\"args\":[\"status\"]}}。".to_string(),
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
    let value: Value =
        serde_json::from_str(response).map_err(|e| format!("LLM 返回了无效 JSON：{}", e))?;
    let value = normalize_planned_command_payload(value);
    let mut planned: PlannedDingTalkCommand = serde_json::from_value(value)
        .map_err(|e| format!("LLM 返回了无法识别的命令 JSON：{}", e))?;

    if planned.workspace_path.is_none() {
        planned.workspace_path = Some(workspace_root.to_string());
    }

    let effective_workspace = planned.workspace_path.as_deref().unwrap_or(workspace_root);
    validate_planned_command(&planned.command, effective_workspace)?;
    Ok(planned)
}

fn normalize_planned_command_payload(value: Value) -> Value {
    let value = normalize_action_tagged_execute_command_payload(value);
    normalize_legacy_browser_actions_payload(value)
}

fn normalize_action_tagged_execute_command_payload(value: Value) -> Value {
    let mut normalized = match value {
        Value::Object(map) => map,
        other => return other,
    };

    if normalized.get("type").is_some() {
        return Value::Object(normalized);
    }

    if normalized.get("action").and_then(Value::as_str) != Some("execute_command") {
        return Value::Object(normalized);
    }

    normalized.insert(
        "type".to_string(),
        Value::String("execute_command".to_string()),
    );
    Value::Object(normalized)
}

fn normalize_legacy_browser_actions_payload(value: Value) -> Value {
    let Some(command) = value.get("command") else {
        return value;
    };

    if command.get("type").and_then(Value::as_str) != Some("browser") {
        return value;
    }

    if command.get("action").is_some() {
        return value;
    }

    let Some(first_action) = command
        .get("actions")
        .and_then(Value::as_array)
        .and_then(|actions| actions.first())
        .cloned()
    else {
        return value;
    };

    let Some(action_name) = first_action.get("type").and_then(Value::as_str) else {
        return value;
    };

    let mut normalized = match value {
        Value::Object(map) => map,
        other => return other,
    };

    let mut command_map = match normalized.remove("command") {
        Some(Value::Object(map)) => map,
        _ => return Value::Object(normalized),
    };

    command_map.remove("actions");
    command_map.insert("action".to_string(), Value::String(action_name.to_string()));

    if let Value::Object(action_fields) = first_action {
        for (key, field_value) in action_fields {
            if key != "type" {
                command_map.insert(key, field_value);
            }
        }
    }

    normalized.insert("command".to_string(), Value::Object(command_map));
    Value::Object(normalized)
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
    let Some(binding) = state
        .hub
        .session_runtime()
        .take_task_binding(&task_result.task_id)
        .await
    else {
        let route = {
            let mut routes = state.dingtalk_routes.write().await;
            routes.remove(&task_result.task_id)
        };
        if route.is_none() {
            return Ok(());
        }

        let completed_task = state.hub.get_completed_task(&task_result.task_id).await;
        let reply_text = build_task_result_reply_text(&state, &task_result).await;

        if let Some(completed_task) = completed_task.as_ref() {
            persist_task_result_memory(&state, completed_task, &reply_text).await;
        }

        let Some(_channel) = state.dingtalk_channel.as_ref() else {
            warn!("Skip DingTalk reply because channel is unavailable");
            return Ok(());
        };
        finalize_dingtalk_reply_handle(&state, &task_result.task_id, route.as_ref().unwrap(), &reply_text).await?;
        info!("Replied DingTalk task result for {}", task_result.task_id);
        return Ok(());
    };

    let session_key = binding.session_key.clone();
    let lane_session_key = session_key.clone();
    let state_for_actor = Arc::clone(&state);
    state
        .hub
        .session_runtime()
        .run_serialized(&lane_session_key, async move {
            let Some(completed_task) = state_for_actor.hub.get_completed_task(&task_result.task_id).await else {
                let reply_text = build_task_result_reply_text(&state_for_actor, &task_result).await;
                state_for_actor
                    .metrics_collector
                    .inc_continuations("task_result_without_completed_task");
                let Some(_channel) = state_for_actor.dingtalk_channel.as_ref() else {
                    warn!("Skip DingTalk reply because channel is unavailable");
                    state_for_actor
                        .hub
                        .session_runtime()
                        .fail_turn(&session_key, Some(&task_result.task_id))
                        .await;
                    return Ok(());
                };
                finalize_dingtalk_reply_handle(&state_for_actor, &task_result.task_id, &binding.route, &reply_text).await?;
                state_for_actor
                    .hub
                    .session_runtime()
                    .complete_turn(&session_key, Some(&task_result.task_id))
                    .await;
                return Ok(());
            };

            state_for_actor
                .hub
                .session_runtime()
                .append_transcript_event(
                    &session_key,
                    TranscriptEventKind::ToolResultObserved,
                    serde_json::to_string(&completed_task.result)
                        .unwrap_or_else(|_| "tool result".to_string()),
                )
                .await;
            state_for_actor
                .metrics_collector
                .inc_continuations("task_result");

            if state_for_actor
                .hub
                .session_runtime()
                .turn_state(&session_key)
                .await
                .map(|turn| turn.cancel_requested)
                .unwrap_or(false)
            {
                if state_for_actor.dingtalk_channel.is_some() {
                    cleanup_dingtalk_reply_handle(&state_for_actor, &task_result.task_id, &binding.route)
                        .await?;
                }
                state_for_actor
                    .hub
                    .session_runtime()
                    .prune_completed_turn_transcript(&session_key, 6)
                    .await;
                info!(
                    "Skip continuation for {} because session turn was cancelled",
                    completed_task.task_id
                );
                return Ok(());
            }

            state_for_actor
                .hub
                .session_runtime()
                .mark_turn_resuming(&session_key, Some(&task_result.task_id))
                .await;
            state_for_actor
                .hub
                .session_runtime()
                .append_transcript_event(
                    &session_key,
                    TranscriptEventKind::TurnResumed,
                    format!("task_result:{}", task_result.task_id),
                )
                .await;

            let step = match continue_task_result(&state_for_actor, &binding, &completed_task).await {
                Ok(step) => {
                    state_for_actor
                        .hub
                        .session_runtime()
                        .reset_planner_retry(&session_key)
                        .await;
                    step
                }
                Err(error) => {
                    warn!(
                        "Failed to continue task result with LLM for {}: {}",
                        completed_task.task_id, error
                    );
                    state_for_actor
                        .metrics_collector
                        .inc_planner_retries("continuation_error");
                    state_for_actor
                        .hub
                        .session_runtime()
                        .append_transcript_event(
                            &session_key,
                            TranscriptEventKind::PlannerRetry,
                            error.to_string(),
                        )
                        .await;
                    let retry_state = state_for_actor
                        .hub
                        .session_runtime()
                        .increment_planner_retry(&session_key)
                        .await;
                    if let Some(retry_state) = retry_state {
                        if retry_state.planner_retry_count <= retry_state.max_planner_retries {
                            let compact_summary = format!(
                                "user_request: {}; last_command: {}; last_result: {}",
                                completed_task.context.intent.clone().unwrap_or_default(),
                                serde_json::to_string(&completed_task.command)
                                    .unwrap_or_else(|_| "{}".to_string()),
                                serde_json::to_string(&completed_task.result)
                                    .unwrap_or_else(|_| "{}".to_string())
                            );
                            state_for_actor
                                .hub
                                .session_runtime()
                                .record_compaction(
                                    &session_key,
                                    compact_summary.clone(),
                                    state_for_actor
                                        .hub
                                        .session_runtime()
                                        .transcript(&session_key)
                                        .await
                                        .map(|transcript| transcript.events.len())
                                        .unwrap_or_default(),
                                )
                                .await;
                            state_for_actor
                                .hub
                                .session_runtime()
                                .append_transcript_event(
                                    &session_key,
                                    TranscriptEventKind::TurnCompacted,
                                    compact_summary,
                                )
                                .await;
                            match continue_task_result(&state_for_actor, &binding, &completed_task).await {
                                Ok(step) => {
                                    state_for_actor
                                        .hub
                                        .session_runtime()
                                        .reset_planner_retry(&session_key)
                                        .await;
                                    step
                                }
                                Err(retry_error) => {
                                    warn!(
                                        "Retry continuation failed for {}: {}",
                                        completed_task.task_id, retry_error
                                    );
                                    PlannedTurnStep::Finalize {
                                        text: summarize_task_result_or_fallback(&state_for_actor, &completed_task).await,
                                    }
                                }
                            }
                        } else {
                            PlannedTurnStep::Finalize {
                                text: summarize_task_result_or_fallback(&state_for_actor, &completed_task).await,
                            }
                        }
                    } else {
                        PlannedTurnStep::Finalize {
                            text: summarize_task_result_or_fallback(&state_for_actor, &completed_task).await,
                        }
                    }
                }
            };
            state_for_actor
                .hub
                .session_runtime()
                .append_transcript_event(
                    &session_key,
                    TranscriptEventKind::AssistantStep,
                    format!("{:?}", step),
                )
                .await;
            let turn_state = state_for_actor
                .hub
                .session_runtime()
                .increment_step_count(&session_key)
                .await
                .ok_or_else(|| "session turn is missing before continuation step".to_string())?;
            if turn_state.step_count > turn_state.max_steps {
                let reply_text = summarize_task_result_or_fallback(&state_for_actor, &completed_task).await;
                state_for_actor
                    .hub
                    .session_runtime()
                    .append_transcript_event(
                        &session_key,
                        TranscriptEventKind::AssistantFinal,
                        reply_text.clone(),
                    )
                    .await;
                persist_task_result_memory(&state_for_actor, &completed_task, &reply_text).await;
                if state_for_actor.dingtalk_channel.is_some() {
                    finalize_dingtalk_reply_handle(&state_for_actor, &task_result.task_id, &binding.route, &reply_text).await?;
                } else {
                    warn!("Skip DingTalk reply because channel is unavailable");
                }
                state_for_actor
                    .hub
                    .session_runtime()
                    .complete_turn(&session_key, Some(&task_result.task_id))
                    .await;
                state_for_actor
                    .hub
                    .session_runtime()
                    .prune_completed_turn_transcript(&session_key, 6)
                    .await;
                return Ok(());
            }

            match step {
                PlannedTurnStep::Finalize { text: reply_text } => {
                    state_for_actor
                        .hub
                        .session_runtime()
                        .append_transcript_event(
                            &session_key,
                            TranscriptEventKind::AssistantFinal,
                            reply_text.clone(),
                        )
                        .await;
                    persist_task_result_memory(&state_for_actor, &completed_task, &reply_text).await;

                    if state_for_actor.dingtalk_channel.is_some() {
                        finalize_dingtalk_reply_handle(&state_for_actor, &task_result.task_id, &binding.route, &reply_text).await?;
                    } else {
                        warn!("Skip DingTalk reply because channel is unavailable");
                    }
                    state_for_actor
                        .hub
                        .session_runtime()
                        .complete_turn(&session_key, Some(&task_result.task_id))
                        .await;
                    state_for_actor
                        .hub
                        .session_runtime()
                        .prune_completed_turn_transcript(&session_key, 6)
                        .await;
                }
                PlannedTurnStep::SubmitTask {
                    command,
                    workspace_path,
                } => {
                    let online_nodes = state_for_actor.hub.get_online_nodes().await;
                    let online_workspace_roots = collect_online_workspace_roots(&online_nodes);
                    let workspace_hint = workspace_path
                        .or_else(|| resolve_default_workspace_root(&online_workspace_roots))
                        .ok_or_else(|| "workspace_path is required for continuation task".to_string())?;
                    let required_capabilities = match &command {
                        Command::Browser(_) => Some(NodeCapabilities {
                            supported_commands: vec![CommandType::Browser],
                            ..NodeCapabilities::default()
                        }),
                        _ => None,
                    };
                    validate_planned_command(&command, &workspace_hint)?;
                    let mut task_context = completed_task.context.clone();
                    task_context.intent = completed_task.context.intent.clone();
                    let tool_call_id = state_for_actor
                        .hub
                        .session_runtime()
                        .begin_tool_call(&session_key, "hub_task")
                        .await
                        .ok_or_else(|| "session turn is missing before continuation task submission".to_string())?;
                    state_for_actor
                        .hub
                        .session_runtime()
                        .append_transcript_event(
                            &session_key,
                            TranscriptEventKind::ToolCallPlanned,
                            format!("{}:{}", tool_call_id, "hub_task"),
                        )
                        .await;
                    let task_id = state_for_actor
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
                    state_for_actor
                        .hub
                        .session_runtime()
                        .bind_task_to_turn(
                            task_id.clone(),
                            TaskContinuationBinding {
                                session_key: binding.session_key.clone(),
                                turn_id: binding.turn_id.clone(),
                                tool_call_id,
                                agent_id: binding.agent_id.clone(),
                                route: binding.route.clone(),
                            },
                        )
                        .await;
                }
                PlannedTurnStep::ExecuteSkill { .. }
                | PlannedTurnStep::ListInstalledSkills
                | PlannedTurnStep::QuerySkill { .. }
                | PlannedTurnStep::InstallSkill { .. } => {
                    let continuation_session_key = parse_dingtalk_binding_session_key(&binding);
                    execute_local_turn_step(
                        &state_for_actor,
                        &binding.route,
                        &continuation_session_key,
                        &binding.agent_id,
                        completed_task.context.intent.as_deref().unwrap_or_default(),
                        &step,
                        &[],
                        binding.route.sender_user_id.as_deref(),
                        binding.route.sender_staff_id.as_deref(),
                        continuation_session_key.team_id.as_deref(),
                        &binding.route.conversation_id,
                        None,
                    )
                    .await?;
                    state_for_actor
                        .hub
                        .session_runtime()
                        .prune_completed_turn_transcript(&session_key, 6)
                        .await;
                }
            }

            info!(
                "Replied DingTalk task result for {} via session actor turn {}",
                task_result.task_id, binding.turn_id
            );
            Ok(())
        })
        .await
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
        source_message_id: inbound.message_id.clone(),
        conversation_type: inbound.conversation_type.clone(),
        sender_user_id: inbound.sender_user_id.clone(),
        sender_staff_id: inbound.sender_staff_id.clone(),
        session_webhook: inbound.session_webhook.clone(),
        session_webhook_expired_time: inbound.session_webhook_expired_time,
        robot_code: inbound.robot_code.clone(),
    };
    let reply_text = format!("执行失败：{}", error_message);
    let reply_handle = Some(create_dingtalk_reply_handle(channel, &route).await?);

    let session_key = build_dingtalk_session_key(
        &inbound.session.channel_user_id,
        inbound.sender_user_id.as_deref(),
        inbound.sender_staff_id.as_deref(),
        inbound.sender_corp_id.as_deref(),
    );
    state
        .hub
        .session_runtime()
        .append_transcript_event(
            &session_key.as_str(),
            TranscriptEventKind::TurnFailed,
            error_message.to_string(),
        )
        .await;

    send_or_finalize_dingtalk_reply(channel, &route, &reply_text, reply_handle, None).await?;

    info!(
        "Replied DingTalk immediate error for conversation {}",
        route.conversation_id
    );
    Ok(())
}

fn resolve_dingtalk_reply_target(route: &DingTalkReplyRoute) -> Option<DingTalkReplyTarget> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let webhook_available = route.session_webhook.as_ref().is_some_and(|_| {
        route
            .session_webhook_expired_time
            .map(|expires_at| now_ms < expires_at)
            .unwrap_or(true)
    });

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

fn format_dingtalk_reply_text(reply_text: &str) -> String {
    let mut formatted_lines = Vec::new();
    let mut previous_blank = false;

    for line in reply_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !previous_blank && !formatted_lines.is_empty() {
                formatted_lines.push(String::new());
                previous_blank = true;
            }
            continue;
        }

        let normalized = if trimmed == "```" {
            None
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            Some(rest.trim().to_string())
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            Some(rest.trim().to_string())
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            Some(rest.trim().to_string())
        } else {
            Some(trimmed.to_string())
        };

        if let Some(normalized) = normalized {
            formatted_lines.push(normalized);
            previous_blank = false;
        }
    }

    while formatted_lines.last().is_some_and(|line| line.is_empty()) {
        formatted_lines.pop();
    }

    formatted_lines.join("\n")
}

fn should_send_dingtalk_immediate_ack(step: &PlannedTurnStep) -> bool {
    matches!(
        step,
        PlannedTurnStep::Finalize { .. }
            | PlannedTurnStep::ExecuteSkill { .. }
            | PlannedTurnStep::ListInstalledSkills
            | PlannedTurnStep::QuerySkill { .. }
            | PlannedTurnStep::InstallSkill { .. }
    )
}

fn should_attach_dingtalk_processing_ack_now(inbound: &DingTalkInboundMessage) -> bool {
    let has_meaningful_text = match &inbound.message.content {
        MessageContent::Text(text) => !text.trim().is_empty(),
        MessageContent::Audio { .. } | MessageContent::Image { .. } => true,
        MessageContent::Structured(data) => {
            data.get("kind").and_then(Value::as_str) == Some("dingtalk_file")
        }
    };

    if !has_meaningful_text {
        return false;
    }

    if inbound.message_id.is_none() && inbound.session_webhook.is_some() {
        return false;
    }

    true
}

fn should_enforce_min_ack_display(step: &PlannedTurnStep) -> bool {
    matches!(step, PlannedTurnStep::Finalize { .. })
}

fn resolve_dingtalk_ai_card_target(route: &DingTalkReplyRoute) -> Option<DingTalkAiCardTarget> {
    if route.robot_code.is_none() {
        return None;
    }

    let is_group = matches!(route.conversation_type.as_deref(), Some("2"));
    if !is_group {
        return None;
    }

    Some(DingTalkAiCardTarget::ImGroup {
        conversation_id: route.conversation_id.clone(),
    })
}

async fn create_dingtalk_reply_handle(
    channel: &DingTalkChannel,
    route: &DingTalkReplyRoute,
) -> Result<DingTalkReplyHandle, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(target) = resolve_dingtalk_ai_card_target(route) {
        if let Some(robot_code) = route.robot_code.as_deref() {
            let Some(card_template_id) = channel.ai_card_template_id() else {
                return Err("DingTalk AI card template id is not configured".into());
            };
            let reply_text = format_dingtalk_reply_text(DINGTALK_PROCESSING_ACK_TEXT);
            let handle = channel
                .create_ai_card(
                    &target,
                    robot_code,
                    card_template_id,
                    "processing",
                    &reply_text,
                )
                .await?;
            return Ok(DingTalkReplyHandle::AiCard {
                handle,
                attached_at: Instant::now(),
            });
        }
    }

    if let (Some(source_message_id), Some(robot_code)) = (
        route.source_message_id.as_deref(),
        route.robot_code.as_deref(),
    ) {
        match channel
            .add_processing_reaction(robot_code, source_message_id, &route.conversation_id)
            .await
        {
            Ok(handle) => {
                return Ok(DingTalkReplyHandle::Reaction {
                    handle,
                    attached_at: Instant::now(),
                })
            },
            Err(error) => {
                warn!(
                    conversation_id = %route.conversation_id,
                    source_message_id = source_message_id,
                    error = %error,
                    "Attach DingTalk processing reaction failed, fallback to legacy processing handle"
                );
            }
        }
    }

    let receipt = match resolve_dingtalk_reply_target(route) {
        Some(DingTalkReplyTarget::SessionWebhook { .. }) => {
            return Ok(DingTalkReplyHandle::Noop);
        }
        Some(DingTalkReplyTarget::GroupConversation { .. }) => None,
        Some(DingTalkReplyTarget::DirectUser { .. }) => None,
        None => {
            warn!(
                conversation_id = %route.conversation_id,
                conversation_type = ?route.conversation_type,
                sender_user_id = ?route.sender_user_id,
                sender_staff_id = ?route.sender_staff_id,
                has_session_webhook = route.session_webhook.is_some(),
                session_webhook_expired_time = ?route.session_webhook_expired_time,
                robot_code = ?route.robot_code,
                "Skip DingTalk processing ack because no reply target could be resolved"
            );
            return Err("No DingTalk reply target could be resolved".into());
        }
    };

    Ok(DingTalkReplyHandle::LegacyTransient {
        receipt,
        attached_at: Instant::now(),
    })
}

async fn cleanup_dingtalk_reply_handle(
    state: &Arc<WebState>,
    task_id: &TaskId,
    route: &DingTalkReplyRoute,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handle = {
        let mut handles = state.dingtalk_reply_handles.write().await;
        handles.remove(task_id)
    };

    let Some(channel) = state.dingtalk_channel.as_ref() else {
        return Ok(());
    };

    cleanup_dingtalk_processing_handle(channel, route, handle, Some(task_id)).await
}

async fn finalize_dingtalk_reply_handle(
    state: &Arc<WebState>,
    task_id: &TaskId,
    route: &DingTalkReplyRoute,
    reply_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handle = {
        let mut handles = state.dingtalk_reply_handles.write().await;
        handles.remove(task_id)
    };

    let Some(channel) = state.dingtalk_channel.as_ref() else {
        return Ok(());
    };

    send_or_finalize_dingtalk_reply(channel, route, reply_text, handle, Some(task_id)).await
}

async fn enforce_dingtalk_ack_min_display(
    reply_handle: Option<&DingTalkReplyHandle>,
    minimum: Duration,
) {
    let Some(attached_at) = reply_handle.and_then(DingTalkReplyHandle::attached_at) else {
        return;
    };
    let elapsed = attached_at.elapsed();
    if elapsed < minimum {
        tokio::time::sleep(minimum - elapsed).await;
    }
}

async fn cleanup_dingtalk_processing_handle(
    channel: &DingTalkChannel,
    _route: &DingTalkReplyRoute,
    reply_handle: Option<DingTalkReplyHandle>,
    task_id: Option<&TaskId>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match reply_handle {
        Some(DingTalkReplyHandle::AiCard { handle, .. }) => {
            if let Err(error) = channel.finalize_ai_card(&handle, DINGTALK_CANCELLED_TEXT).await {
                warn!(task_id = ?task_id, error = %error, "Finalize DingTalk AI card failed during cleanup");
            }
        }
        Some(DingTalkReplyHandle::Reaction { handle, .. }) => {
            if let Err(error) = channel.recall_processing_reaction(&handle).await {
                warn!(task_id = ?task_id, error = %error, "Recall DingTalk processing reaction failed during cleanup");
            }
        }
        Some(DingTalkReplyHandle::LegacyTransient { receipt, .. }) => {
            let outcome = channel.clear_processing_ack(receipt.as_ref()).await?;
            if matches!(outcome, DingTalkTransientClearOutcome::Unsupported) {
                info!(task_id = ?task_id, "DingTalk transient ack clear is unsupported during cleanup");
            }
        }
        Some(DingTalkReplyHandle::Noop) | None => {}
    }

    Ok(())
}

async fn send_or_finalize_dingtalk_reply(
    channel: &DingTalkChannel,
    route: &DingTalkReplyRoute,
    reply_text: &str,
    reply_handle: Option<DingTalkReplyHandle>,
    task_id: Option<&TaskId>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match reply_handle {
        Some(DingTalkReplyHandle::AiCard { handle, .. }) => {
            if let Err(error) = channel.finalize_ai_card(&handle, reply_text).await {
                warn!(task_id = ?task_id, error = %error, "Finalize DingTalk AI card failed, fallback to markdown reply");
                send_dingtalk_reply(channel, route, reply_text).await?;
            }
        }
        Some(DingTalkReplyHandle::Reaction { handle, .. }) => {
            if let Err(error) = channel.recall_processing_reaction(&handle).await {
                warn!(task_id = ?task_id, error = %error, "Recall DingTalk processing reaction failed; continue with final reply");
            }
            send_dingtalk_reply(channel, route, reply_text).await?;
        }
        Some(DingTalkReplyHandle::LegacyTransient { receipt, .. }) => {
            let outcome = channel.clear_processing_ack(receipt.as_ref()).await?;
            if matches!(outcome, DingTalkTransientClearOutcome::Unsupported) {
                info!(task_id = ?task_id, "DingTalk transient ack clear is unsupported; continue with final reply");
            }
            send_dingtalk_reply(channel, route, reply_text).await?;
        }
        Some(DingTalkReplyHandle::Noop) => {
            if reply_text != DINGTALK_PROCESSING_ACK_TEXT {
                send_dingtalk_reply(channel, route, reply_text).await?;
            }
        }
        None => {
            send_dingtalk_reply(channel, route, reply_text).await?;
        }
    }

    Ok(())
}

async fn send_dingtalk_reply(
    channel: &DingTalkChannel,
    route: &DingTalkReplyRoute,
    reply_text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let reply_text = format_dingtalk_reply_text(reply_text);

    match resolve_dingtalk_reply_target(route) {
        Some(DingTalkReplyTarget::SessionWebhook {
            webhook,
            at_user_ids,
        }) => {
            channel
                .reply_via_session_webhook_markdown(&webhook, "uHorse", &reply_text, &at_user_ids)
                .await?;
        }
        Some(DingTalkReplyTarget::GroupConversation { conversation_id }) => {
            channel
                .send_group_markdown_message(&conversation_id, "uHorse", &reply_text)
                .await?;
        }
        Some(DingTalkReplyTarget::DirectUser { user_id }) => {
            channel.send_markdown(&user_id, "uHorse", &reply_text).await?;
        }
        None => {
            warn!(
                conversation_id = %route.conversation_id,
                conversation_type = ?route.conversation_type,
                sender_user_id = ?route.sender_user_id,
                sender_staff_id = ?route.sender_staff_id,
                has_session_webhook = route.session_webhook.is_some(),
                session_webhook_expired_time = ?route.session_webhook_expired_time,
                robot_code = ?route.robot_code,
                "Skip DingTalk reply because no reply target could be resolved"
            );
            return Err("No DingTalk reply target could be resolved".into());
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

fn parse_dingtalk_binding_session_key(binding: &TaskContinuationBinding) -> SessionKey {
    build_dingtalk_session_key(
        &binding.route.conversation_id,
        binding.route.sender_user_id.as_deref(),
        binding.route.sender_staff_id.as_deref(),
        None,
    )
}

async fn continue_task_result(
    state: &Arc<WebState>,
    binding: &TaskContinuationBinding,
    completed_task: &CompletedTask,
) -> Result<PlannedTurnStep, Box<dyn std::error::Error + Send + Sync>> {
    let Some(llm_client) = state.llm_client.as_ref() else {
        return Err("LLM client is not configured".into());
    };

    let session_key = parse_dingtalk_binding_session_key(binding);
    let compacted_summary = state
        .hub
        .session_runtime()
        .turn_state(&binding.session_key)
        .await
        .and_then(|turn| turn.compacted_summary);

    let response = llm_client
        .chat_completion(build_task_result_continuation_messages(
            binding,
            completed_task,
            compacted_summary.as_deref(),
        ))
        .await?;

    let planned = parse_next_step_response(
        state,
        response.trim(),
        completed_task.context.intent.as_deref().unwrap_or_default(),
        &binding.agent_id,
        &session_key,
    )
    .await?;

    Ok(planned)
}

async fn parse_next_step_response(
    state: &Arc<WebState>,
    response: &str,
    original_text: &str,
    agent_id: &str,
    session_key: &SessionKey,
) -> Result<PlannedTurnStep, Box<dyn std::error::Error + Send + Sync>> {
    let trimmed = response.trim();
    if let Some(json_payload) = extract_first_json_object(trimmed) {
        if let Ok(step) = serde_json::from_str::<AgentDecision>(&json_payload) {
            return Ok(planned_step_from_agent_decision(step));
        }

        if let Ok(planned) = parse_planned_command(&json_payload, "") {
            return Ok(PlannedTurnStep::SubmitTask {
                command: planned.command,
                workspace_path: planned.workspace_path,
            });
        }
    }

    if !trimmed.is_empty() {
        if let Ok(planned) = parse_planned_command(trimmed, "") {
            return Ok(PlannedTurnStep::SubmitTask {
                command: planned.command,
                workspace_path: planned.workspace_path,
            });
        }

        return Ok(PlannedTurnStep::Finalize {
            text: trimmed.to_string(),
        });
    }

    plan_next_dingtalk_step(state, original_text, agent_id, session_key).await
}

fn build_task_result_continuation_messages(
    binding: &TaskContinuationBinding,
    completed_task: &CompletedTask,
    compacted_summary: Option<&str>,
) -> Vec<ChatMessage> {
    let mut messages = vec![
        ChatMessage::system(
            "你是 uHorse Hub 的 ReAct continuation planner。上一个 tool 调用已经完成，现在你要基于用户原始请求和最新 observation 决定下一步。你必须只输出一个 JSON 对象或最终中文回复。允许的 JSON 结构与首轮决策一致：direct_reply / execute_command / execute_skill / list_installed_skills / query_skill / install_skill。如果 observation 已经足够回答用户，就直接输出最终中文回复；如果还需要继续操作，就输出下一步 JSON。不要暴露 turn/tool_call/observation 等内部术语。".to_string(),
        ),
        ChatMessage::user(format!(
            "agent_id: {}\nsession_key: {}\nturn_id: {}\ntool_call_id: {}\nuser_request: {}\nexecuted_command: {}\nobservation: {}\n请给出下一步。",
            binding.agent_id,
            binding.session_key,
            binding.turn_id,
            binding.tool_call_id,
            completed_task.context.intent.clone().unwrap_or_default(),
            serde_json::to_string(&completed_task.command).unwrap_or_else(|_| "{}".to_string()),
            serde_json::to_string(&completed_task.result).unwrap_or_else(|_| "{}".to_string())
        )),
    ];
    if let Some(summary) = compacted_summary {
        if !summary.trim().is_empty() {
            messages.push(ChatMessage::system(format!(
                "当前 turn 已压缩历史摘要：{}",
                summary
            )));
        }
    }
    messages
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

    let skill_names = state.agent_runtime.skills.read().await.list_all_names();
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
                skill_names: skill_names.clone(),
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
                    skill_names: state.agent_runtime.skills.read().await.list_all_names(),
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
        .read()
        .await
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

async fn install_runtime_skill(
    State(state): State<Arc<WebState>>,
    Json(request): Json<SkillInstallRequest>,
) -> (StatusCode, Json<ApiResponse<SkillInstallResponse>>) {
    let actor = SkillInstallActor {
        channel: "http_api",
        sender_user_id: None,
        sender_staff_id: None,
        sender_corp_id: None,
    };

    match install_skill_from_request(&state, actor, request).await {
        Ok(response) => (StatusCode::CREATED, Json(ApiResponse::success(response))),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn refresh_runtime_skill(
    State(state): State<Arc<WebState>>,
) -> (StatusCode, Json<ApiResponse<SkillRefreshResponse>>) {
    match refresh_runtime_skills(state.agent_runtime.as_ref()).await {
        Ok(skill_count) => {
            info!(action = "skill_refresh", channel = "http_api", result = "success", skill_count, "Refreshed runtime skills");
            (
                StatusCode::OK,
                Json(ApiResponse::success(SkillRefreshResponse { skill_count })),
            )
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn get_runtime_skill(
    State(state): State<Arc<WebState>>,
    Path(skill_name): Path<String>,
    Query(query): Query<SkillRuntimeQuery>,
) -> (StatusCode, Json<ApiResponse<SkillRuntimeDetail>>) {
    let skills = state.agent_runtime.skills.read().await;
    let entry = match query.source_layer.as_deref() {
        Some(source_layer) => skills.get_entry_by_source(
            &skill_name,
            source_layer,
            query.source_scope.as_deref(),
        ),
        None => skills.get_any_entry(&skill_name),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartPairingPayload {
    node_id: String,
    #[serde(default)]
    node_name: Option<String>,
    #[serde(default)]
    device_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PairingRequestResponse {
    request_id: String,
    node_id: String,
    node_name: String,
    device_type: String,
    pairing_code: String,
    status: String,
    created_at: u64,
    expires_at: u64,
    bound_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AccountStatusResponse {
    node_id: String,
    pairing_enabled: bool,
    bound_user_id: Option<String>,
    pairing: Option<PairingRequestResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CancelPairingPayload {
    request_id: String,
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

fn pairing_enabled(state: &WebState) -> bool {
    state.pairing_manager.is_some()
}

fn pairing_status_label(status: &PairingStatus) -> &'static str {
    match status {
        PairingStatus::Pending => "pending",
        PairingStatus::AwaitingConfirmation => "awaiting_confirmation",
        PairingStatus::Paired => "paired",
        PairingStatus::Rejected => "rejected",
        PairingStatus::Expired => "expired",
        PairingStatus::Cancelled => "cancelled",
    }
}

fn pairing_request_response(request: PairingRequest) -> PairingRequestResponse {
    PairingRequestResponse {
        request_id: request.request_id,
        node_id: request.device_id.to_string(),
        node_name: request.device_name,
        device_type: request.device_type,
        pairing_code: request.pairing_code,
        status: pairing_status_label(&request.status).to_string(),
        created_at: request.created_at,
        expires_at: request.expires_at,
        bound_user_id: request.user_id,
    }
}

fn resolve_pairing_command(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.len() == 6 && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(trimmed);
    }

    let normalized = trimmed
        .strip_prefix("绑定码")
        .or_else(|| trimmed.strip_prefix("pair"))
        .or_else(|| trimmed.strip_prefix("bind"))
        .map(str::trim)
        .unwrap_or(trimmed);

    if normalized.len() == 6 && normalized.chars().all(|ch| ch.is_ascii_digit()) {
        Some(normalized)
    } else {
        None
    }
}

fn resolve_dingtalk_skill_install_request(text: &str) -> Option<SkillInstallRequest> {
    let trimmed = text.trim();
    let normalized = trimmed
        .strip_prefix("安装技能")
        .or_else(|| trimmed.strip_prefix("install skill"))
        .map(str::trim)?;
    if normalized.is_empty() {
        return None;
    }

    let mut parts = normalized.split_whitespace();
    let package = parts.next()?.to_string();
    let download_url = parts.next()?.to_string();
    let version = parts.next().map(str::to_string);

    Some(SkillInstallRequest {
        source_type: SkillInstallSourceType::Skillhub,
        package,
        version,
        download_url,
        target_layer: SkillInstallTargetLayer::Global,
        target_scope: None,
        attachment_download_code: None,
        attachment_file_name: None,
    })
}

fn build_installed_skills_reply(skills: &[SkillRuntimeSummary]) -> String {
    if skills.is_empty() {
        return "当前还没有安装任何 Skill。".to_string();
    }

    let lines = skills
        .iter()
        .map(|skill| format!("- {}：{}", skill.name, skill.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!("当前已安装 {} 个 Skill：\n{}", skills.len(), lines)
}

fn looks_like_dingtalk_skill_install_intent(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    trimmed.starts_with("安装技能")
        || trimmed.starts_with("安装 ")
        || trimmed.contains("帮我安装")
        || trimmed.contains("请帮我安装")
        || trimmed.contains("请安装")
        || lower.starts_with("install skill")
        || lower.starts_with("install ")
        || lower.contains("please install")
}

fn normalize_dingtalk_skill_install_query(text: &str) -> Option<String> {
    let candidate = text
        .trim()
        .trim_end_matches('。')
        .trim_end_matches('！')
        .trim_end_matches('!')
        .trim_end_matches('？')
        .trim_end_matches('?')
        .trim();
    let candidate = candidate
        .strip_suffix("技能")
        .or_else(|| candidate.strip_suffix("skill"))
        .or_else(|| candidate.strip_suffix("Skill"))
        .unwrap_or(candidate)
        .trim();

    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn is_likely_pure_skill_name(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let blocked_fragments = [
        "帮我", "请帮我", "请", "看看", "列出", "已经安装", "已安装", "安装", "please",
        "install", "list", "show", "query",
    ];
    if blocked_fragments.iter().any(|fragment| {
        trimmed.contains(fragment) || lower.contains(&fragment.to_ascii_lowercase())
    }) {
        return false;
    }

    trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || ch.is_ascii_whitespace()
            || matches!(ch, '-' | '_' | '.' | '技' | '能')
    })
}

fn parse_dingtalk_skill_install_search_query(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if looks_like_dingtalk_skill_install_intent(trimmed) {
        let without_prefix = trimmed
            .strip_prefix("帮我")
            .or_else(|| trimmed.strip_prefix("请帮我"))
            .or_else(|| trimmed.strip_prefix("请"))
            .unwrap_or(trimmed)
            .trim();

        let lower = without_prefix.to_ascii_lowercase();
        let candidate = if let Some(rest) = without_prefix.strip_prefix("安装") {
            rest.trim()
        } else if let Some(index) = lower.find("install") {
            without_prefix[index + "install".len()..].trim()
        } else if let Some(index) = without_prefix.find('装') {
            without_prefix[index + '装'.len_utf8()..].trim()
        } else {
            without_prefix
        };

        return normalize_dingtalk_skill_install_query(candidate);
    }

    if is_likely_pure_skill_name(trimmed) {
        return normalize_dingtalk_skill_install_query(trimmed);
    }

    None
}

fn infer_skillhub_slug_from_query(query: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in query.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn configured_skillhub_search_url(state: &WebState) -> String {
    state
        .app_config
        .channels
        .dingtalk
        .as_ref()
        .and_then(|config| config.skillhub_search_url.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("UHORSE_SKILLHUB_SEARCH_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_SKILLHUB_SEARCH_URL.to_string())
}

fn configured_skillhub_download_url_template(state: &WebState) -> String {
    state
        .app_config
        .channels
        .dingtalk
        .as_ref()
        .and_then(|config| config.skillhub_download_url_template.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| SKILLHUB_PRIMARY_DOWNLOAD_URL_TEMPLATE.to_string())
}

async fn search_skillhub_skill(
    state: &WebState,
    query: &str,
) -> Result<Option<SkillhubSearchEntry>, Box<dyn std::error::Error + Send + Sync>> {
    #[derive(Debug, Deserialize)]
    struct SkillhubSearchResponse {
        data: SkillhubSearchData,
    }

    #[derive(Debug, Deserialize)]
    struct SkillhubSearchData {
        #[serde(default)]
        skills: Vec<SkillhubSearchItem>,
    }

    #[derive(Debug, Deserialize)]
    struct SkillhubSearchItem {
        slug: String,
        #[serde(rename = "displayName")]
        display_name: Option<String>,
        name: Option<String>,
        title: Option<String>,
        version: Option<String>,
    }

    let client = build_skillhub_http_client()?;
    let search_url = configured_skillhub_search_url(state);
    let response = client
        .get(&search_url)
        .query(&[
            ("page", "1"),
            ("pageSize", "24"),
            ("sortBy", "score"),
            ("order", "desc"),
            ("keyword", query),
        ])
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Skill 搜索失败：HTTP {}", status).into());
    }

    let payload: SkillhubSearchResponse = response.json().await.map_err(|error| {
        format!(
            "Skill 搜索响应解析失败：query={} url={} status={} error={}",
            query, search_url, status, error
        )
    })?;
    let Some(first) = payload.data.skills.into_iter().next() else {
        return Ok(None);
    };

    Ok(Some(SkillhubSearchEntry {
        slug: first.slug.clone(),
        name: first
            .display_name
            .filter(|value| !value.trim().is_empty())
            .or(first.title)
            .or(first.name)
            .unwrap_or(first.slug),
        version: first.version.filter(|value| !value.trim().is_empty()),
    }))
}

fn build_skillhub_download_url(state: &WebState, slug: &str) -> String {
    configured_skillhub_download_url_template(state).replace("{slug}", slug)
}

fn is_allowed_skillhub_download_url(download_url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(download_url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    SKILLHUB_OFFICIAL_HOSTS
        .iter()
        .any(|allowed| host.eq_ignore_ascii_case(allowed))
}

async fn resolve_dingtalk_skill_install_intent(
    state: &WebState,
    text: &str,
    pending_attachments: &[PendingDingTalkAttachment],
) -> Result<Option<DingtalkSkillInstallIntent>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(request) = build_pending_attachment_install_request(text, pending_attachments) {
        return Ok(Some(DingtalkSkillInstallIntent::ExplicitCommand(request)));
    }

    if let Some(request) = resolve_dingtalk_skill_install_request(text) {
        if !is_allowed_skillhub_download_url(&request.download_url) {
            return Err("仅允许安装 SkillHub 官方下载地址。".into());
        }
        return Ok(Some(DingtalkSkillInstallIntent::ExplicitCommand(request)));
    }

    let Some(query) = parse_dingtalk_skill_install_search_query(text) else {
        return Ok(None);
    };

    let entry = match search_skillhub_skill(state, &query).await {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            let Some(slug) = infer_skillhub_slug_from_query(&query) else {
                return Ok(Some(DingtalkSkillInstallIntent::NaturalLanguageNoMatch(query)));
            };
            warn!(
                query = %query,
                inferred_slug = %slug,
                "SkillHub search returned no match, fallback to inferred slug"
            );
            SkillhubSearchEntry {
                slug,
                name: query,
                version: None,
            }
        }
        Err(error) => {
            let Some(slug) = infer_skillhub_slug_from_query(&query) else {
                return Err(error);
            };
            warn!(
                query = %query,
                inferred_slug = %slug,
                error = %error,
                "SkillHub search failed, fallback to inferred slug"
            );
            SkillhubSearchEntry {
                slug,
                name: query,
                version: None,
            }
        }
    };

    Ok(Some(DingtalkSkillInstallIntent::NaturalLanguage(entry)))
}

async fn process_dingtalk_pairing_command(
    state: &Arc<WebState>,
    inbound: &DingTalkInboundMessage,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let Some(pairing_manager) = state.pairing_manager.as_ref() else {
        return Ok(None);
    };

    let Some(text) = inbound.message.content.as_text().map(str::trim) else {
        return Ok(None);
    };
    let Some(code) = resolve_pairing_command(text) else {
        return Ok(None);
    };

    let Some(user_id) = inbound
        .sender_user_id
        .clone()
        .or_else(|| inbound.sender_staff_id.clone())
    else {
        return Err("DingTalk sender identity is missing".into());
    };

    let reply_text = match pairing_manager.confirm_pairing(code, user_id.clone()).await {
        Ok(device) => {
            state
                .hub
                .notification_bindings()
                .set_binding(device.id.as_str(), user_id)
                .await;
            format!("绑定成功，设备 {} 已关联当前 DingTalk 账号。", device.name)
        }
        Err(error) => {
            let message = error.to_string();
            if message.contains("expired") {
                "绑定失败：绑定码已过期，请在桌面端重新发起绑定。".to_string()
            } else if message.contains("Invalid pairing code") {
                "绑定失败：绑定码无效，请检查后重试。".to_string()
            } else {
                format!("绑定失败：{}", message)
            }
        }
    };

    Ok(Some(reply_text))
}

async fn try_handle_dingtalk_pairing_command(
    state: &Arc<WebState>,
    inbound: &DingTalkInboundMessage,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(reply_text) = process_dingtalk_pairing_command(state, inbound).await? else {
        return Ok(false);
    };

    let route = reply_route_from_inbound(inbound);
    let Some(channel) = state.dingtalk_channel.as_ref() else {
        return Err("DingTalk channel is not configured".into());
    };

    send_dingtalk_reply(channel, &route, &reply_text).await?;
    Ok(true)
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

async fn authorize_account_api_node(
    state: &Arc<WebState>,
    headers: &HeaderMap,
    requested_node_id: &str,
) -> Result<(), (StatusCode, &'static str)> {
    let Some(security_manager) = state.hub.security_manager() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Security manager not configured",
        ));
    };

    let Some(auth_header) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return Err((StatusCode::UNAUTHORIZED, "Missing Authorization header"));
    };

    let Some(token) = auth_header.strip_prefix(BEARER_AUTH_PREFIX) else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization header"));
    };

    let token = token.trim();
    if token.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "Missing bearer token"));
    }

    let authenticated_node_id = match security_manager
        .node_authenticator()
        .verify_token(token)
        .await
    {
        Ok(node_id) => node_id,
        Err(error) => {
            warn!(
                requested_node_id,
                error = %error,
                "account api token verification failed"
            );
            return Err((StatusCode::UNAUTHORIZED, "Token verification failed"));
        }
    };

    if authenticated_node_id.as_str() != requested_node_id {
        warn!(
            requested_node_id,
            authenticated_node_id = authenticated_node_id.as_str(),
            "account api node_id mismatch"
        );
        return Err((
            StatusCode::FORBIDDEN,
            "Token node_id does not match requested node_id",
        ));
    }

    Ok(())
}

async fn start_account_pairing(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(payload): Json<StartPairingPayload>,
) -> (StatusCode, Json<ApiResponse<PairingRequestResponse>>) {
    if let Err((status, message)) =
        authorize_account_api_node(&state, &headers, &payload.node_id).await
    {
        return (status, Json(ApiResponse::error(message)));
    }

    let Some(pairing_manager) = state.pairing_manager.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Pairing manager not configured")),
        );
    };

    let device_id = uhorse_core::DeviceId::from_string(payload.node_id.clone());
    let device_name = payload.node_name.unwrap_or_else(|| payload.node_id.clone());
    let device_type = payload.device_type.unwrap_or_else(|| "desktop".to_string());

    match pairing_manager
        .initiate_pairing(device_id, device_name, device_type)
        .await
    {
        Ok(request) => (
            StatusCode::OK,
            Json(ApiResponse::success(pairing_request_response(request))),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn cancel_account_pairing(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(payload): Json<CancelPairingPayload>,
) -> (StatusCode, Json<ApiResponse<&'static str>>) {
    let Some(pairing_manager) = state.pairing_manager.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::error("Pairing manager not configured")),
        );
    };

    let request = match pairing_manager
        .get_pairing_request(&payload.request_id)
        .await
    {
        Ok(request) => request,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(&error.to_string())),
            )
        }
    };

    if let Err((status, message)) =
        authorize_account_api_node(&state, &headers, request.device_id.as_str()).await
    {
        return (status, Json(ApiResponse::error(message)));
    }

    match pairing_manager.cancel_pairing(&payload.request_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(ApiResponse::success("Pairing cancelled")),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(&error.to_string())),
        ),
    }
}

async fn get_account_status(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<AccountStatusResponse>>) {
    if let Err((status, message)) = authorize_account_api_node(&state, &headers, &node_id).await {
        return (status, Json(ApiResponse::error(message)));
    }

    let bound_user_id = state
        .hub
        .notification_bindings()
        .get_user_id(&node_id)
        .await;

    let pairing = if let Some(pairing_manager) = state.pairing_manager.as_ref() {
        match pairing_manager
            .get_pairing_status(&uhorse_core::DeviceId::from_string(node_id.clone()))
            .await
        {
            Ok(PairingStatus::Paired) => None,
            Ok(_) => pairing_manager
                .list_pending_requests()
                .await
                .ok()
                .and_then(|requests| {
                    requests
                        .into_iter()
                        .find(|request| request.device_id.as_str() == node_id)
                })
                .map(pairing_request_response),
            Err(_) => None,
        }
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(ApiResponse::success(AccountStatusResponse {
            node_id,
            pairing_enabled: pairing_enabled(&state),
            bound_user_id,
            pairing,
        })),
    )
}

async fn delete_account_binding(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
) -> (StatusCode, Json<ApiResponse<&'static str>>) {
    if let Err((status, message)) = authorize_account_api_node(&state, &headers, &node_id).await {
        return (status, Json(ApiResponse::error(message)));
    }

    state
        .hub
        .notification_bindings()
        .unbind(node_id.clone())
        .await;

    if let Some(pairing_manager) = state.pairing_manager.as_ref() {
        let _ = uhorse_core::DeviceManager::unpair_device(
            pairing_manager.as_ref(),
            &uhorse_core::DeviceId::from_string(node_id),
        )
        .await;
    }

    (
        StatusCode::OK,
        Json(ApiResponse::success("Binding removed")),
    )
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
    let task_id = uhorse_protocol::TaskId::from_string(&task_id);
    if let Some(binding) = state.hub.session_runtime().find_task_binding(&task_id).await {
        state
            .hub
            .session_runtime()
            .cancel_turn(&binding.session_key)
            .await;
    }

    match state.hub.cancel_task(&task_id, "User cancelled").await {
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

    state
        .metrics_collector
        .inc_approval_resumes(if approved { "approved" } else { "rejected" });

    let _ = log_audit_event(AuditEvent {
        timestamp: chrono::Utc::now().timestamp() as u64,
        level: if approved { AuditLevel::Info } else { AuditLevel::Warn },
        category: AuditCategory::Session,
        actor: Some(payload.responder.clone()),
        action: if approved {
            "approval_approved".to_string()
        } else {
            "approval_rejected".to_string()
        },
        target: Some(request_id.clone()),
        details: Some(serde_json::json!({
            "task_id": existing_request.metadata.get("task_id").and_then(|value| value.as_str()),
            "node_id": existing_request.metadata.get("node_id").and_then(|value| value.as_str()),
            "reason": payload.reason,
        })),
        session_id: existing_request
            .metadata
            .get("context")
            .and_then(|value| value.get("session_id"))
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
    })
    .await;

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

    if let Some(task_id) = existing_request
        .metadata
        .get("task_id")
        .and_then(|value| value.as_str())
    {
        if let Some(binding) = state
            .hub
            .session_runtime()
            .find_task_binding(&TaskId::from_string(task_id))
            .await
        {
            state
                .hub
                .session_runtime()
                .append_transcript_event(
                    &binding.session_key,
                    if approved {
                        TranscriptEventKind::ApprovalApproved
                    } else {
                        TranscriptEventKind::ApprovalRejected
                    },
                    format!("request_id={}; responder={}", request_id, payload.responder),
                )
                .await;
        }
    }

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
    let mailbox_snapshots = state.hub.session_runtime().mailbox_snapshots().await;
    let waiting_for_tool_turns = mailbox_snapshots
        .iter()
        .filter(|snapshot| {
            snapshot
                .turn
                .as_ref()
                .map(|turn| turn.status == TurnStatus::WaitingForTool)
                .unwrap_or(false)
        })
        .count() as u64;
    let waiting_for_approval_turns = mailbox_snapshots
        .iter()
        .filter(|snapshot| {
            snapshot
                .turn
                .as_ref()
                .map(|turn| turn.status == TurnStatus::WaitingForApproval)
                .unwrap_or(false)
        })
        .count() as u64;
    state
        .metrics_collector
        .set_runtime_mailbox_state(
            mailbox_snapshots.len() as u64,
            waiting_for_tool_turns,
            waiting_for_approval_turns,
        )
        .await;

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

async fn get_runtime_diagnostics(
    State(state): State<Arc<WebState>>,
) -> Json<ApiResponse<HubRuntimeDiagnosticsResponse>> {
    let session_mailboxes = state.hub.session_runtime().mailbox_snapshots().await;
    let waiting_for_tool_turns = session_mailboxes
        .iter()
        .filter(|snapshot| {
            snapshot
                .turn
                .as_ref()
                .map(|turn| turn.status == TurnStatus::WaitingForTool)
                .unwrap_or(false)
        })
        .count();
    let waiting_for_approval_turns = session_mailboxes
        .iter()
        .filter(|snapshot| {
            snapshot
                .turn
                .as_ref()
                .map(|turn| turn.status == TurnStatus::WaitingForApproval)
                .unwrap_or(false)
        })
        .count();
    let active_task_bindings = session_mailboxes
        .iter()
        .map(|snapshot| snapshot.active_task_binding_count)
        .sum();

    Json(ApiResponse::success(HubRuntimeDiagnosticsResponse {
        session_mailboxes,
        waiting_for_tool_turns,
        waiting_for_approval_turns,
        active_task_bindings,
    }))
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
    use uhorse_observability::AuditLogger;
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

    async fn create_test_runtime_with_skill_only(
        skill_name: &str,
        skill_toml: &str,
    ) -> Arc<WebAgentRuntime> {
        let base_dir = tempdir().unwrap().keep();
        let runtime_root = base_dir.join("agent-runtime");
        write_test_skill(&runtime_root, skill_name, skill_toml).await;
        Arc::new(init_default_agent_runtime(runtime_root).await.unwrap())
    }

    async fn create_test_runtime_with_skill(
        skill_name: &str,
        skill_toml: &str,
        llm_response: &str,
    ) -> (Arc<WebAgentRuntime>, Arc<WebState>) {
        let runtime = create_test_runtime_with_skill_only(skill_name, skill_toml).await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: llm_response.to_string(),
            })),
            None,
            runtime.clone(),
        ));

        (runtime, state)
    }

    async fn write_test_skill(runtime_root: &std::path::Path, skill_name: &str, skill_toml: &str) {
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
    }

    fn build_test_skill_archive(skill_name: &str, skill_toml: &str) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let skill_md = format!(
            "---\nname: {}\nversion: 1.0.0\ndescription: {} skill\nauthor: test\nparameters: []\npermissions: []\n---\n",
            skill_name, skill_name
        );

        let mut skill_md_header = tar::Header::new_gnu();
        skill_md_header.set_size(skill_md.len() as u64);
        skill_md_header.set_mode(0o644);
        skill_md_header.set_cksum();
        builder
            .append_data(
                &mut skill_md_header,
                format!("{}/SKILL.md", skill_name),
                skill_md.as_bytes(),
            )
            .unwrap();

        let mut skill_toml_header = tar::Header::new_gnu();
        skill_toml_header.set_size(skill_toml.len() as u64);
        skill_toml_header.set_mode(0o644);
        skill_toml_header.set_cksum();
        builder
            .append_data(
                &mut skill_toml_header,
                format!("{}/skill.toml", skill_name),
                skill_toml.as_bytes(),
            )
            .unwrap();

        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap()
    }

    fn build_test_skill_zip_archive_with_skill_md(
        skill_md: &str,
        skill_toml: &str,
    ) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("SKILL.md", options).unwrap();
        std::io::Write::write_all(&mut writer, skill_md.as_bytes()).unwrap();
        writer.start_file("skill.toml", options).unwrap();
        std::io::Write::write_all(&mut writer, skill_toml.as_bytes()).unwrap();
        writer.finish().unwrap();
        bytes.into_inner()
    }

    fn build_test_skill_zip_archive(skill_toml: &str) -> Vec<u8> {
        let skill_md = "---\nname: agent-browser\nversion: 1.0.0\ndescription: agent browser skill\nauthor: test\nparameters: []\npermissions: []\n---\n";
        build_test_skill_zip_archive_with_skill_md(skill_md, skill_toml)
    }

    async fn start_test_archive_server(bytes: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
        let bytes = Arc::new(bytes);
        let app = Router::new().route(
            "/skill.tar.gz",
            get(move || {
                let bytes = bytes.clone();
                async move { bytes.as_ref().clone() }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://{}/skill.tar.gz", address), handle)
    }

    async fn start_skillhub_search_server(
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new().route(
            "/api/v1/search",
            get(move || async move {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    body.to_string(),
                )
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://{}/api/v1/search", address), handle)
    }

    #[derive(Debug, Clone)]
    struct CapturedSessionWebhookRequest {
        auth_header: Option<String>,
        body: serde_json::Value,
    }

    async fn start_session_webhook_server(
    ) -> (
        String,
        Arc<tokio::sync::Mutex<Vec<CapturedSessionWebhookRequest>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/hook",
            axum::routing::post({
                let requests = requests.clone();
                move |
                    headers: axum::http::HeaderMap,
                    axum::Json(body): axum::Json<serde_json::Value>,
                | {
                    let requests = requests.clone();
                    async move {
                        requests.lock().await.push(CapturedSessionWebhookRequest {
                            auth_header: headers
                                .get("x-acs-dingtalk-access-token")
                                .and_then(|value| value.to_str().ok())
                                .map(str::to_string),
                            body,
                        });
                        ([(axum::http::header::CONTENT_TYPE, "application/json")], "{}")
                    }
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (format!("http://{}/hook", address), requests, handle)
    }

    async fn create_skill_install_test_state_with_skillhub(
        installers: Vec<DingTalkSkillInstaller>,
        skillhub_search_url: Option<String>,
        skillhub_download_url_template: Option<String>,
    ) -> (TempDir, Arc<WebState>) {
        let workspace = tempdir().unwrap();
        let runtime_root = workspace.path().join("agent-runtime");
        let runtime = Arc::new(init_default_agent_runtime(runtime_root).await.unwrap());
        let (hub, _rx) = Hub::new(HubConfig::default());
        let mut config = UHorseConfig::default();
        config.channels.dingtalk = Some(uhorse_config::DingTalkConfig {
            app_key: "key".to_string(),
            app_secret: "secret".to_string(),
            agent_id: 1,
            ai_card_template_id: None,
            skillhub_search_url,
            skillhub_download_url_template,
            notification_bindings: vec![],
            skill_installers: installers,
        });
        let state = Arc::new(WebState::new_with_runtime_and_config(
            Arc::new(config),
            Arc::new(hub),
            None,
            None,
            None,
            runtime,
        ));

        (workspace, state)
    }

    async fn create_skill_install_test_state(
        installers: Vec<DingTalkSkillInstaller>,
    ) -> (TempDir, Arc<WebState>) {
        create_skill_install_test_state_with_skillhub(installers, None, None).await
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

    struct SequenceLlmClient {
        responses: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl LLMClient for SequenceLlmClient {
        async fn chat_completion(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no llm response configured")
            }
            Ok(responses.remove(0))
        }
    }

    struct FailingLlmClient;

    #[async_trait::async_trait]
    impl LLMClient for FailingLlmClient {
        async fn chat_completion(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Err(anyhow::anyhow!("llm failed"))
        }
    }

    struct FailThenSucceedLlmClient {
        responses: std::sync::Mutex<Vec<Result<String>>>,
    }

    #[async_trait::async_trait]
    impl LLMClient for FailThenSucceedLlmClient {
        async fn chat_completion(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                anyhow::bail!("no llm response configured")
            }
            responses.remove(0)
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
        let pairing_manager = Arc::new(DevicePairingManager::new());
        let state = Arc::new(WebState::new_with_pairing(
            hub.clone(),
            None,
            None,
            Some(pairing_manager),
        ));
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
        create_registered_node_test_state_with_llm(None).await
    }

    async fn create_registered_node_test_state_with_llm(
        llm_client: Option<Arc<dyn LLMClient>>,
    ) -> (
        Arc<WebState>,
        Arc<Hub>,
        uhorse_protocol::NodeId,
        tokio::sync::mpsc::Receiver<HubToNode>,
        TempDir,
    ) {
        let workspace = tempdir().unwrap();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_pairing(
            hub.clone(),
            None,
            llm_client,
            Some(Arc::new(DevicePairingManager::new())),
        ));
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

    async fn issue_test_node_token(
        state: &Arc<WebState>,
        node_id: &uhorse_protocol::NodeId,
    ) -> String {
        state
            .hub
            .security_manager()
            .unwrap()
            .node_authenticator()
            .authenticate_node(node_id, "test-credentials")
            .await
            .unwrap()
            .access_token
    }

    async fn post_json<T: Serialize>(
        app: Router,
        path: &str,
        payload: &T,
    ) -> (StatusCode, serde_json::Value) {
        post_json_with_auth(app, path, payload, None).await
    }

    async fn post_json_with_auth<T: Serialize>(
        app: Router,
        path: &str,
        payload: &T,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = auth_token {
            request = request.header(
                header::AUTHORIZATION,
                format!("{}{}", BEARER_AUTH_PREFIX, token),
            );
        }
        let request = request
            .body(Body::from(serde_json::to_vec(payload).unwrap()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    async fn get_json(app: Router, path: &str) -> (StatusCode, serde_json::Value) {
        get_json_with_auth(app, path, None).await
    }

    async fn get_json_with_auth(
        app: Router,
        path: &str,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder().method("GET").uri(path);
        if let Some(token) = auth_token {
            request = request.header(
                header::AUTHORIZATION,
                format!("{}{}", BEARER_AUTH_PREFIX, token),
            );
        }
        let request = request.body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    async fn delete_json_with_auth(
        app: Router,
        path: &str,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder().method("DELETE").uri(path);
        if let Some(token) = auth_token {
            request = request.header(
                header::AUTHORIZATION,
                format!("{}{}", BEARER_AUTH_PREFIX, token),
            );
        }
        let request = request.body(Body::empty()).unwrap();
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
        create_pending_approval_with_task(state, node_id, request_id, "task-approval-web").await
    }

    async fn create_pending_approval_with_task(
        state: &Arc<WebState>,
        node_id: &uhorse_protocol::NodeId,
        request_id: &str,
        task_id: &str,
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
                    "task_id": task_id,
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
    async fn test_health_check_supports_custom_configured_path() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new(Arc::new(hub), None, None);
        let app = create_router_with_health_path(state, "/custom-health");

        let (custom_status, body) = get_json(app.clone(), "/custom-health").await;
        let (legacy_status, _, _) = get_text(app, "/api/health").await;

        assert_eq!(custom_status, StatusCode::OK);
        assert_eq!(body["status"], json!("healthy"));
        assert_eq!(legacy_status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_health_check_can_be_disabled() {
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = WebState::new(Arc::new(hub), None, None);
        let app = create_router_with_health_config(
            state,
            &HealthConfig {
                enabled: false,
                path: "/api/health".to_string(),
                verbose: false,
            },
        );

        let (status, _, _) = get_text(app, "/api/health").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
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
        assert!(body.contains("uhorse_runtime_mailbox_sessions 0"));
        assert!(body.contains("uhorse_runtime_waiting_for_tool_turns 0"));
        assert!(body.contains("uhorse_runtime_waiting_for_approval_turns 0"));
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

    #[test]
    fn test_parse_planned_command_accepts_minimal_shell_command() {
        let planned = parse_planned_command(
            r#"{"command":{"type":"shell","command":"pwd"}}"#,
            "/tmp/workspace",
        )
        .unwrap();

        assert_eq!(planned.workspace_path.as_deref(), Some("/tmp/workspace"));
        match planned.command {
            Command::Shell(shell) => {
                assert_eq!(shell.command, "pwd");
                assert!(shell.args.is_empty());
                assert_eq!(shell.cwd, None);
                assert!(shell.env.is_empty());
                assert_eq!(shell.timeout.as_secs(), 60);
                assert!(shell.capture_stderr);
            }
            other => panic!("expected shell command, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_planned_command_normalizes_legacy_browser_actions_payload() {
        let workspace = "/tmp/workspace";
        let response = r#"{"type":"execute_command","command":{"type":"browser","actions":[{"type":"open_system","url":"https://www.baidu.com"}]},"workspace_path":"/tmp/workspace"}"#;

        let planned = parse_planned_command(response, workspace).unwrap();

        assert_eq!(planned.workspace_path.as_deref(), Some(workspace));
        match planned.command {
            Command::Browser(BrowserCommand::OpenSystem { url }) => {
                assert_eq!(url, "https://www.baidu.com");
            }
            other => panic!("expected browser open_system command, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_planned_command_accepts_action_execute_command_tag() {
        let workspace = "/tmp/workspace";
        let response = r#"{"action":"execute_command","command":{"type":"shell","command":"pwd"},"workspace_path":"/tmp/workspace"}"#;

        let planned = parse_planned_command(response, workspace).unwrap();

        assert_eq!(planned.workspace_path.as_deref(), Some(workspace));
        assert!(matches!(planned.command, Command::Shell(_)));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_extracts_shell_execute_command_json_from_wrapped_text() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let response = format!(
            "好的，执行如下：\n{{\"type\":\"execute_command\",\"command\":{{\"type\":\"shell\",\"command\":\"pwd\"}},\"workspace_path\":\"{}\"}}",
            workspace_path
        );
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient { response })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-shell-json");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-shell-command");

        let decision = decide_dingtalk_action(&state, "列出当前目录", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::Shell(_),
                workspace_path: Some(ref resolved_workspace_path),
            } if resolved_workspace_path == &workspace_path
        ));
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
            status: TaskStatus::Completed,
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
            status: TaskStatus::Completed,
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

    #[tokio::test]
    async fn test_plan_dingtalk_command_maps_baidu_request_to_open_system() {
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"command":{"type":"browser","action":"open_system","url":"https://www.baidu.com"}}"#.to_string(),
            })),
        ));

        let session_key = SessionKey::new("dingtalk", "user-1");
        let decision = plan_dingtalk_command(
            &state,
            "帮我访问百度",
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
                assert_eq!(url, "https://www.baidu.com");
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
            None,
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

    #[test]
    fn test_should_force_dingtalk_command_planning_detects_weather_queries() {
        assert!(should_force_dingtalk_command_planning("今天北京天气如何？"));
        assert!(should_force_dingtalk_command_planning("帮我查一下最新汇率"));
        assert!(!should_force_dingtalk_command_planning("你好"));
        assert!(!should_force_dingtalk_command_planning("Browser Use 技能怎么用"));
    }

    #[test]
    fn test_direct_reply_for_forced_planning_without_workspace_returns_offline_hint() {
        assert_eq!(
            direct_reply_for_forced_planning_without_workspace("今天北京天气如何？").as_deref(),
            Some("当前 Node Desktop 不在线，暂时无法执行实时查询。请先启动 Node Desktop，启动后我就可以继续帮你查询。")
        );
        assert_eq!(direct_reply_for_forced_planning_without_workspace("你好"), None);
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_replans_weather_query_when_llm_returns_plain_text() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(SequenceLlmClient {
                responses: std::sync::Mutex::new(vec![
                    "我现在无法实时查询北京天气。".to_string(),
                    r#"{"command":{"type":"browser","action":"open_system","url":"https://wttr.in/Beijing?format=3"}}"#.to_string(),
                ]),
            })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-weather-replan");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities {
                supported_commands: vec![CommandType::Browser],
                ..NodeCapabilities::default()
            },
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-weather-replan");

        let decision = decide_dingtalk_action(&state, "今天北京天气如何？", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::Browser(BrowserCommand::OpenSystem { ref url }),
                workspace_path: Some(ref resolved_workspace_path),
            } if url == "https://wttr.in/Beijing?format=3" && resolved_workspace_path == &workspace_path
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_replans_weather_query_when_llm_returns_direct_reply_json() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(SequenceLlmClient {
                responses: std::sync::Mutex::new(vec![
                    r#"{"type":"direct_reply","text":"我现在无法实时查询北京天气。"}"#.to_string(),
                    r#"{"command":{"type":"browser","action":"open_system","url":"https://wttr.in/Beijing?format=3"}}"#.to_string(),
                ]),
            })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-weather-replan-json");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities {
                supported_commands: vec![CommandType::Browser],
                ..NodeCapabilities::default()
            },
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-weather-replan-json");

        let decision = decide_dingtalk_action(&state, "今天北京天气如何？", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::Browser(BrowserCommand::OpenSystem { ref url }),
                workspace_path: Some(ref resolved_workspace_path),
            } if url == "https://wttr.in/Beijing?format=3" && resolved_workspace_path == &workspace_path
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_returns_offline_hint_for_weather_query_without_workspace() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"direct_reply","text":"我现在无法实时查询北京天气。"}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-weather-offline");

        let decision = decide_dingtalk_action(&state, "今天北京天气如何？", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::DirectReply { text }
                if text == "当前 Node Desktop 不在线，暂时无法执行实时查询。请先启动 Node Desktop，启动后我就可以继续帮你查询。"
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_parses_list_installed_skills_json() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"list_installed_skills"}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-list-skills");

        let decision = decide_dingtalk_action(
            &state,
            "帮我列出已经安装的技能",
            "main",
            &session_key,
        )
        .await
        .unwrap();

        assert!(matches!(decision, AgentDecision::ListInstalledSkills));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_parses_install_skill_json() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"install_skill","query":"Browser Use"}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-install-skill");

        let decision = decide_dingtalk_action(&state, "帮我安装 Browser Use 技能", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::InstallSkill { query } if query == "Browser Use"
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_parses_query_skill_json() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"query_skill","skill_name":"browser-use"}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-query-skill");

        let decision = decide_dingtalk_action(&state, "Browser Use 技能怎么用", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::QuerySkill { skill_name } if skill_name == "browser-use"
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_extracts_execute_command_json_from_wrapped_text() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let response = format!(
            "好的，执行如下：\n{{\"type\":\"execute_command\",\"command\":{{\"type\":\"file\",\"action\":\"exists\",\"path\":\"{}/Cargo.toml\"}},\"workspace_path\":\"{}\"}}",
            workspace_path,
            workspace_path
        );
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient { response })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-json");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-command");

        let decision = decide_dingtalk_action(&state, "列出当前目录", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::File(FileCommand::Exists { .. }),
                workspace_path: Some(ref resolved_workspace_path),
            } if resolved_workspace_path == &workspace_path
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_extracts_action_execute_command_json_from_wrapped_text() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let response = format!(
            "好的，继续执行：\n{{\"action\":\"execute_command\",\"command\":{{\"type\":\"file\",\"action\":\"exists\",\"path\":\"{}/Cargo.toml\"}},\"workspace_path\":\"{}\"}}",
            workspace_path,
            workspace_path
        );
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient { response })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-action-json");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities::default(),
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-action-command");

        let decision = decide_dingtalk_action(&state, "列出当前目录", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::File(FileCommand::Exists { .. }),
                workspace_path: Some(ref resolved_workspace_path),
            } if resolved_workspace_path == &workspace_path
        ));
    }

    #[tokio::test]
    async fn test_decide_dingtalk_action_parses_legacy_browser_actions_json_without_leaking_reply() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let workspace = tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();
        let response = format!(
            r#"{{"type":"execute_command","command":{{"type":"browser","actions":[{{"type":"open_system","url":"https://www.baidu.com"}}]}},"workspace_path":"{}"}}"#,
            workspace_path
        );
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient { response })),
            None,
            runtime,
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-browser-json");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        hub.message_router().register_node_sender(node_id.clone(), tx).await;
        hub.handle_node_connection(
            node_id,
            "test-node".to_string(),
            NodeCapabilities {
                supported_commands: vec![CommandType::Browser],
                ..NodeCapabilities::default()
            },
            WorkspaceInfo {
                workspace_id: None,
                name: "workspace".to_string(),
                path: workspace_path.clone(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            vec![],
        )
        .await
        .unwrap();
        let session_key = SessionKey::new("dingtalk", "user-browser-command");

        let decision = decide_dingtalk_action(&state, "帮我访问百度", "main", &session_key)
            .await
            .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::ExecuteCommand {
                command: Command::Browser(BrowserCommand::OpenSystem { ref url }),
                workspace_path: Some(ref resolved_workspace_path),
            } if url == "https://www.baidu.com" && resolved_workspace_path == &workspace_path
        ));
    }

    #[tokio::test]
    async fn test_reply_task_result_continues_in_session_actor() {
        let llm_client = Arc::new(SequenceLlmClient {
            responses: std::sync::Mutex::new(vec![
                "{\"type\":\"execute_command\",\"command\":{\"type\":\"file\",\"action\":\"exists\",\"path\":\"Cargo.toml\"},\"workspace_path\":null}".to_string(),
            ]),
        });
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(llm_client),
            Some(Arc::new(DevicePairingManager::new())),
            runtime,
        ));
        let binding = TaskContinuationBinding {
            session_key: "dingtalk:user-react:corp-react".to_string(),
            turn_id: "turn-1".to_string(),
            tool_call_id: "tool-1".to_string(),
            agent_id: "main".to_string(),
            route: DingTalkReplyRoute {
                conversation_id: "conv-react".to_string(),
                source_message_id: None,
                conversation_type: Some("1".to_string()),
                sender_user_id: Some("user-react".to_string()),
                sender_staff_id: Some("staff-react".to_string()),
                session_webhook: None,
                session_webhook_expired_time: None,
                robot_code: None,
            },
        };
        let completed_task = CompletedTask {
            task_id: TaskId::from_string("task-1"),
            command: Command::File(FileCommand::Exists {
                path: "README.md".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("user-react"),
                uhorse_protocol::SessionId::from_string("dingtalk:user-react:corp-react"),
                "dingtalk",
            )
            .with_intent("检查 README 是否存在并告诉我结果"),
            priority: Priority::Normal,
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            status: TaskStatus::Completed,
            result: CommandResult::success(CommandOutput::text("README.md exists")),
        };

        let step = continue_task_result(&state, &binding, &completed_task)
            .await
            .unwrap();

        assert!(matches!(
            step,
            PlannedTurnStep::SubmitTask {
                command: Command::File(FileCommand::Exists { path }),
                workspace_path: None,
            } if path == "Cargo.toml"
        ));
    }

    #[tokio::test]
    async fn test_reply_task_result_dispatches_follow_up_task_and_updates_turn_state() {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, mut task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(SequenceLlmClient {
                responses: std::sync::Mutex::new(vec![
                    format!(
                        r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/README.md"}},"workspace_path":"{}"}}"#,
                        workspace_root, workspace_root
                    ),
                    "{\"type\":\"execute_command\",\"command\":{\"type\":\"file\",\"action\":\"exists\",\"path\":\"Cargo.toml\"},\"workspace_path\":null}".to_string(),
                ]),
            })),
            None,
            runtime,
        ));

        let node_id = uhorse_protocol::NodeId::from_string("node-react-follow-up");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id.clone(),
            "react-node".to_string(),
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
                MessageContent::Text("先检查 README，再检查 Cargo.toml".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-react-follow-up".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let first_assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let first_task_id = match first_assignment {
            HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                match command {
                    Command::File(FileCommand::Exists { path }) => {
                        assert_eq!(path, format!("{}/README.md", workspace_root));
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                assert_eq!(
                    context.intent.as_deref(),
                    Some("先检查 README，再检查 Cargo.toml")
                );
                task_id
            }
            other => panic!("unexpected message: {:?}", other),
        };

        hub.handle_node_message(
            &node_id,
            NodeToHub::TaskResult {
                message_id: uhorse_protocol::MessageId::new(),
                task_id: first_task_id.clone(),
                result: CommandResult::success(CommandOutput::text("README exists")),
                metrics: uhorse_protocol::ExecutionMetrics {
                    duration_ms: 1,
                    cpu_time_ms: 0,
                    peak_memory_mb: 0,
                    bytes_read: 0,
                    bytes_written: 0,
                    network_requests: 0,
                },
            },
        )
        .await
        .unwrap();

        let task_result = tokio::time::timeout(std::time::Duration::from_secs(1), task_result_rx.recv())
            .await
            .unwrap()
            .unwrap();
        reply_task_result(state.clone(), task_result).await.unwrap();

        let second_assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let second_task_id = match second_assignment {
            HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                match command {
                    Command::File(FileCommand::Exists { path }) => {
                        assert_eq!(path, "Cargo.toml");
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                assert_eq!(
                    context.intent.as_deref(),
                    Some("先检查 README，再检查 Cargo.toml")
                );
                task_id
            }
            other => panic!("unexpected message: {:?}", other),
        };

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let turn = state
            .hub
            .session_runtime()
            .turn_state(&session_key.as_str())
            .await
            .unwrap();
        assert_eq!(turn.step_count, 2);
        assert_eq!(turn.status, crate::session_runtime::TurnStatus::WaitingForTool);
        assert_eq!(
            turn.tool_call.as_ref().and_then(|tool_call| tool_call.task_id.as_ref()),
            Some(&second_task_id)
        );

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript
            .events
            .iter()
            .any(|event| event.kind == crate::session_runtime::TranscriptEventKind::ToolResultObserved));
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::AssistantStep
                && event.content.contains("SubmitTask")
        }));
        assert_eq!(
            transcript
                .events
                .iter()
                .filter(|event| event.kind == crate::session_runtime::TranscriptEventKind::ToolCallDispatched)
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn test_reply_task_result_stops_when_turn_exceeds_max_steps() {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, mut task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(SequenceLlmClient {
                responses: std::sync::Mutex::new(vec![
                    format!(
                        r#"{{"type":"execute_command","command":{{"type":"file","action":"exists","path":"{}/README.md"}},"workspace_path":"{}"}}"#,
                        workspace_root, workspace_root
                    ),
                    "README exists".to_string(),
                ]),
            })),
            None,
            runtime.clone(),
        ));

        let node_id = uhorse_protocol::NodeId::from_string("node-react-max-steps");
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(node_id.clone(), tx)
            .await;
        hub.handle_node_connection(
            node_id.clone(),
            "react-node".to_string(),
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
                MessageContent::Text("一直继续执行".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-react-max-steps".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let first_assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let first_task_id = match first_assignment {
            HubToNode::TaskAssignment { task_id, .. } => task_id,
            other => panic!("unexpected message: {:?}", other),
        };

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        for _ in 0..3 {
            state
                .hub
                .session_runtime()
                .increment_step_count(&session_key.as_str())
                .await
                .unwrap();
        }

        hub.handle_node_message(
            &node_id,
            NodeToHub::TaskResult {
                message_id: uhorse_protocol::MessageId::new(),
                task_id: first_task_id.clone(),
                result: CommandResult::success(CommandOutput::text("README exists")),
                metrics: uhorse_protocol::ExecutionMetrics {
                    duration_ms: 1,
                    cpu_time_ms: 0,
                    peak_memory_mb: 0,
                    bytes_read: 0,
                    bytes_written: 0,
                    network_requests: 0,
                },
            },
        )
        .await
        .unwrap();

        let task_result = tokio::time::timeout(std::time::Duration::from_secs(1), task_result_rx.recv())
            .await
            .unwrap()
            .unwrap();
        reply_task_result(state.clone(), task_result).await.unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
                .await
                .is_err()
        );

        let turn = state
            .hub
            .session_runtime()
            .turn_state(&session_key.as_str())
            .await
            .unwrap();
        assert_eq!(turn.step_count, 5);
        assert_eq!(turn.status, crate::session_runtime::TurnStatus::Completed);

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::AssistantFinal
                && event.content == "README exists"
        }));

        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** 一直继续执行"));
        assert!(history.contains("**Assistant:** README exists"));
    }

    #[tokio::test]
    async fn test_reply_task_result_skips_continuation_when_turn_cancelled() {
        let runtime = create_test_runtime().await;
        let (hub, mut task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let channel = Arc::new(DingTalkChannel::new(
            "key".to_string(),
            "secret".to_string(),
            1,
            None,
        ));
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            Some(channel),
            Some(Arc::new(FailingLlmClient)),
            None,
            runtime,
        ));
        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let turn_id = state
            .hub
            .session_runtime()
            .start_turn(&session_key.as_str(), "取消当前执行")
            .await;
        let tool_call_id = state
            .hub
            .session_runtime()
            .begin_tool_call(&session_key.as_str(), "hub_task")
            .await
            .unwrap();
        let task_id = TaskId::from_string("task-cancelled");
        state
            .hub
            .session_runtime()
            .bind_task_to_turn(
                task_id.clone(),
                TaskContinuationBinding {
                    session_key: session_key.as_str().to_string(),
                    turn_id,
                    tool_call_id,
                    agent_id: "main".to_string(),
                    route: DingTalkReplyRoute {
                        conversation_id: "conv-cancelled".to_string(),
                        source_message_id: None,
                        conversation_type: Some("2".to_string()),
                        sender_user_id: Some("actual-user".to_string()),
                        sender_staff_id: Some("staff-1".to_string()),
                        session_webhook: None,
                        session_webhook_expired_time: None,
                        robot_code: Some("robot-1".to_string()),
                    },
                },
            )
            .await;
        state
            .hub
            .session_runtime()
            .cancel_turn(&session_key.as_str())
            .await;

        let completed_task = CompletedTask {
            task_id: task_id.clone(),
            command: Command::File(FileCommand::Exists {
                path: "README.md".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("actual-user"),
                uhorse_protocol::SessionId::from_string(session_key.as_str()),
                "dingtalk",
            )
            .with_intent("取消当前执行"),
            priority: Priority::Normal,
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            status: TaskStatus::Completed,
            result: CommandResult::success(CommandOutput::text("README exists")),
        };
        hub.task_scheduler()
            .insert_completed_task_for_test(completed_task)
            .await;
        state.dingtalk_reply_handles.write().await.insert(
            task_id.clone(),
            DingTalkReplyHandle::LegacyTransient {
                receipt: Some(DingTalkTransientMessageReceipt::unsupported()),
                attached_at: Instant::now(),
            },
        );

        reply_task_result(
            state.clone(),
            TaskResult {
                task_id: task_id.clone(),
                node_id: uhorse_protocol::NodeId::from_string("node-1"),
                result: CommandResult::success(CommandOutput::text("README exists")),
            },
        )
        .await
        .unwrap();

        assert!(tokio::time::timeout(std::time::Duration::from_millis(100), task_result_rx.recv())
            .await
            .is_err());

        let turn = state
            .hub
            .session_runtime()
            .turn_state(&session_key.as_str())
            .await
            .unwrap();
        assert!(turn.cancel_requested);
        assert_eq!(turn.status, crate::session_runtime::TurnStatus::Cancelled);
        assert_eq!(turn.planner_retry_count, 0);

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript
            .events
            .iter()
            .any(|event| event.kind == crate::session_runtime::TranscriptEventKind::TurnCancelled));
        assert!(transcript
            .events
            .iter()
            .any(|event| event.kind == crate::session_runtime::TranscriptEventKind::ToolResultObserved));
        assert!(!transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::AssistantStep
                && event.content.contains("SubmitTask")
        }));
        assert!(!state.dingtalk_reply_handles.read().await.contains_key(&task_id));
    }

    #[tokio::test]
    async fn test_reply_task_result_records_compaction_and_retries_once() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(FailThenSucceedLlmClient {
                responses: std::sync::Mutex::new(vec![
                    Err(anyhow::anyhow!("llm failed")),
                    Ok("重试后总结完成".to_string()),
                ]),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let turn_id = state
            .hub
            .session_runtime()
            .start_turn(&session_key.as_str(), "检查 README 后总结")
            .await;
        let tool_call_id = state
            .hub
            .session_runtime()
            .begin_tool_call(&session_key.as_str(), "hub_task")
            .await
            .unwrap();
        let task_id = TaskId::from_string("task-retry");
        state
            .hub
            .session_runtime()
            .bind_task_to_turn(
                task_id.clone(),
                TaskContinuationBinding {
                    session_key: session_key.as_str().to_string(),
                    turn_id,
                    tool_call_id,
                    agent_id: "main".to_string(),
                    route: DingTalkReplyRoute {
                        conversation_id: "conv-retry".to_string(),
                        source_message_id: None,
                        conversation_type: Some("2".to_string()),
                        sender_user_id: Some("actual-user".to_string()),
                        sender_staff_id: Some("staff-1".to_string()),
                        session_webhook: None,
                        session_webhook_expired_time: None,
                        robot_code: Some("robot-1".to_string()),
                    },
                },
            )
            .await;

        let completed_task = CompletedTask {
            task_id: task_id.clone(),
            command: Command::File(FileCommand::Exists {
                path: "README.md".to_string(),
            }),
            context: TaskContext::new(
                UserId::from_string("actual-user"),
                uhorse_protocol::SessionId::from_string(session_key.as_str()),
                "dingtalk",
            )
            .with_intent("检查 README 后总结"),
            priority: Priority::Normal,
            node_id: uhorse_protocol::NodeId::from_string("node-1"),
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            status: TaskStatus::Completed,
            result: CommandResult::success(CommandOutput::text("README exists")),
        };
        state
            .hub
            .task_scheduler()
            .insert_completed_task_for_test(completed_task)
            .await;

        reply_task_result(
            state.clone(),
            TaskResult {
                task_id: task_id.clone(),
                node_id: uhorse_protocol::NodeId::from_string("node-1"),
                result: CommandResult::success(CommandOutput::text("README exists")),
            },
        )
        .await
        .unwrap();

        let turn = state
            .hub
            .session_runtime()
            .turn_state(&session_key.as_str())
            .await
            .unwrap();
        assert_eq!(turn.status, crate::session_runtime::TurnStatus::Completed);
        assert_eq!(turn.planner_retry_count, 0);
        assert!(turn.compacted_summary.is_some());
        assert!(turn.pruned_event_count >= 2);

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript
            .events
            .iter()
            .any(|event| event.kind == crate::session_runtime::TranscriptEventKind::TurnCompacted));
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::AssistantFinal
                && event.content == "重试后总结完成"
        }));
    }

    #[test]
    fn test_project_transcript_messages_includes_intermediate_events() {
        let transcript = crate::session_runtime::SessionTranscript {
            turn_id: "turn-1".to_string(),
            events: vec![
                crate::session_runtime::TranscriptEvent {
                    seq: 1,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::UserMessage,
                    content: "请检查 README".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 2,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::AssistantStep,
                    content: "SubmitTask".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 3,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::ToolCallDispatched,
                    content: "tool-1:task-1".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 4,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::ToolResultObserved,
                    content: "README exists".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 5,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::ApprovalApproved,
                    content: "request_id=req-1; responder=admin".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 6,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::TurnResumed,
                    content: "task_result:task-1".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 7,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::PlannerRetry,
                    content: "llm failed".to_string(),
                },
                crate::session_runtime::TranscriptEvent {
                    seq: 8,
                    created_at: chrono::Utc::now(),
                    kind: crate::session_runtime::TranscriptEventKind::AssistantFinal,
                    content: "README 存在。".to_string(),
                },
            ],
        };

        let projected = project_transcript_messages(&transcript);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].user_message, "请检查 README");
        assert!(projected[0]
            .assistant_message
            .contains("[assistant_step] SubmitTask"));
        assert!(projected[0]
            .assistant_message
            .contains("[tool_call_dispatched] tool-1:task-1"));
        assert!(projected[0]
            .assistant_message
            .contains("[tool_result_observed] README exists"));
        assert!(projected[0]
            .assistant_message
            .contains("[approval_approved] request_id=req-1; responder=admin"));
        assert!(projected[0]
            .assistant_message
            .contains("[turn_resumed] task_result:task-1"));
        assert!(projected[0]
            .assistant_message
            .contains("[planner_retry] llm failed"));
        assert!(projected[0].assistant_message.contains("README 存在。"));
    }

    #[tokio::test]
    async fn test_cancel_task_api_marks_session_turn_cancelled_for_running_task() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            None,
            None,
            runtime,
        ));
        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let turn_id = state
            .hub
            .session_runtime()
            .start_turn(&session_key.as_str(), "取消任务")
            .await;
        let tool_call_id = state
            .hub
            .session_runtime()
            .begin_tool_call(&session_key.as_str(), "hub_task")
            .await
            .unwrap();

        let task_id = hub.task_scheduler().generate_task_id();
        state
            .hub
            .session_runtime()
            .bind_task_to_turn(
                task_id.clone(),
                TaskContinuationBinding {
                    session_key: session_key.as_str().to_string(),
                    turn_id,
                    tool_call_id,
                    agent_id: "main".to_string(),
                    route: DingTalkReplyRoute {
                        conversation_id: "conv-cancel-api".to_string(),
                        source_message_id: None,
                        conversation_type: Some("2".to_string()),
                        sender_user_id: Some("actual-user".to_string()),
                        sender_staff_id: Some("staff-1".to_string()),
                        session_webhook: None,
                        session_webhook_expired_time: None,
                        robot_code: Some("robot-1".to_string()),
                    },
                },
            )
            .await;
        let node_id = uhorse_protocol::NodeId::from_string("node-cancel-api");
        hub.task_scheduler()
            .insert_running_task_for_test(
                crate::task_scheduler::RunningTask {
                    task_id: task_id.clone(),
                    command: Command::File(FileCommand::Exists {
                        path: "README.md".to_string(),
                    }),
                    context: TaskContext::new(
                        UserId::from_string("actual-user"),
                        uhorse_protocol::SessionId::from_string(session_key.as_str()),
                        "dingtalk",
                    )
                    .with_intent("取消任务"),
                    priority: Priority::Normal,
                    node_id: node_id.clone(),
                    started_at: chrono::Utc::now(),
                    timeout_at: chrono::Utc::now() + chrono::Duration::seconds(30),
                    retry_count: 0,
                },
            )
            .await;

        let app = create_router((*state).clone());
        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{}/cancel", task_id))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!("Task cancelled"));

        let turn = state
            .hub
            .session_runtime()
            .turn_state(&session_key.as_str())
            .await
            .unwrap();
        assert!(turn.cancel_requested);
        assert_eq!(turn.status, crate::session_runtime::TurnStatus::Cancelled);

        let task_status = hub.get_task_status(&task_id).await.unwrap();
        assert_eq!(task_status.status, TaskStatus::Cancelled);
        assert_eq!(task_status.error.as_deref(), Some("Task cancelled"));

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript
            .events
            .iter()
            .any(|event| event.kind == crate::session_runtime::TranscriptEventKind::TurnCancelled));
    }

    #[tokio::test]
    async fn test_persist_task_result_memory_updates_history_and_today_memory() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
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
            status: TaskStatus::Completed,
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
        let state = WebState::new_with_runtime(Arc::new(hub), None, None, None, Arc::new(runtime));
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
        let state = WebState::new_with_runtime(Arc::new(hub), None, None, None, Arc::new(runtime));
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
            None,
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
            None,
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
            None,
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
            None,
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
            None,
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        let error = submit_dingtalk_task(&state, inbound, None).await.unwrap_err();
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
            None,
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
        assert!(!state.dingtalk_reply_handles.read().await.contains_key(&task_id));
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_dispatches_weather_query_after_direct_reply_json_replan() {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(SequenceLlmClient {
                responses: std::sync::Mutex::new(vec![
                    r#"{"type":"direct_reply","text":"我现在无法实时查询北京天气。"}"#.to_string(),
                    format!(
                        r#"{{"command":{{"type":"browser","action":"open_system","url":"https://wttr.in/Beijing?format=3"}},"workspace_path":"{}"}}"#,
                        workspace_root
                    ),
                ]),
            })),
            None,
            runtime,
        ));

        let browser_node_id = uhorse_protocol::NodeId::from_string("node-weather-browser");
        let (browser_tx, mut browser_rx) = tokio::sync::mpsc::channel(8);
        hub.message_router()
            .register_node_sender(browser_node_id.clone(), browser_tx)
            .await;
        hub.handle_node_connection(
            browser_node_id.clone(),
            "browser-node".to_string(),
            NodeCapabilities {
                supported_commands: vec![CommandType::Browser],
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
            "weather-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Text("今天北京天气如何？".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-weather".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
                        assert_eq!(url, "https://wttr.in/Beijing?format=3");
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                assert_eq!(context.intent.as_deref(), Some("今天北京天气如何？"));
                assert_eq!(context.session_id.as_str(), "dingtalk:actual-user:corp-1");
                task_id
            }
            other => panic!("unexpected message: {:?}", other),
        };

        let routes = state.dingtalk_routes.read().await;
        assert!(routes.contains_key(&task_id));
        drop(routes);

    }

    #[tokio::test]
    async fn test_reply_task_result_ignores_unsupported_transient_ack_clear() {
        let runtime = create_test_runtime().await;
        let (hub, _task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"execute_command","command":{"type":"file","action":"exists","path":"Cargo.toml"},"workspace_path":"/tmp"}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let task_id = TaskId::new();
        state.dingtalk_reply_handles.write().await.insert(
            task_id.clone(),
            DingTalkReplyHandle::LegacyTransient {
                receipt: Some(DingTalkTransientMessageReceipt::unsupported()),
                attached_at: Instant::now(),
            },
        );

        let route = DingTalkReplyRoute {
            conversation_id: "conv-1".to_string(),
            source_message_id: Some("msg-1".to_string()),
            conversation_type: Some("2".to_string()),
            sender_user_id: None,
            sender_staff_id: None,
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };
        finalize_dingtalk_reply_handle(&state, &task_id, &route, "done")
            .await
            .unwrap();
        assert!(!state.dingtalk_reply_handles.read().await.contains_key(&task_id));
    }

    #[tokio::test]
    async fn test_create_dingtalk_reply_handle_returns_noop_for_session_webhook_route() {
        let channel = DingTalkChannel::new("key".to_string(), "secret".to_string(), 1, None);
        let route = DingTalkReplyRoute {
            conversation_id: "conv-session".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: Some("https://example.com/hook".to_string()),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
            robot_code: Some("robot-1".to_string()),
        };

        let handle = create_dingtalk_reply_handle(&channel, &route)
            .await
            .unwrap();

        assert!(matches!(handle, DingTalkReplyHandle::Noop));
    }

    #[tokio::test]
    async fn test_parse_next_step_response_parses_legacy_browser_actions_json_as_submit_task() {
        let session_key = SessionKey::new("dingtalk", "user-next-step");
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(Arc::new(hub), None, None));
        let response = r#"{"type":"execute_command","command":{"type":"browser","actions":[{"type":"open_system","url":"https://www.baidu.com"}]},"workspace_path":"/tmp/workspace"}"#;

        let step = parse_next_step_response(
            &state,
            response,
            "帮我访问百度",
            "main",
            &session_key,
        )
        .await
        .unwrap();

        assert!(matches!(
            step,
            PlannedTurnStep::SubmitTask {
                command: Command::Browser(BrowserCommand::OpenSystem { ref url }),
                workspace_path: Some(ref workspace_path),
            } if url == "https://www.baidu.com" && workspace_path == "/tmp/workspace"
        ));
    }

    #[tokio::test]
    async fn test_parse_next_step_response_parses_action_execute_command_as_submit_task() {
        let session_key = SessionKey::new("dingtalk", "user-next-step-action");
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new(Arc::new(hub), None, None));
        let response = r#"{"action":"execute_command","command":{"type":"browser","action":"open_system","url":"https://example.com/article"},"workspace_path":"/tmp/workspace"}"#;

        let step = parse_next_step_response(
            &state,
            response,
            "帮我看看文章",
            "main",
            &session_key,
        )
        .await
        .unwrap();

        assert!(matches!(
            step,
            PlannedTurnStep::SubmitTask {
                command: Command::Browser(BrowserCommand::OpenSystem { ref url }),
                workspace_path: Some(ref workspace_path),
            } if url == "https://example.com/article" && workspace_path == "/tmp/workspace"
        ));
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_dispatches_baidu_open_system_to_browser_node() {
        let runtime = create_test_runtime().await;
        let workspace = tempdir().unwrap();
        let workspace_root = workspace.path().to_string_lossy().to_string();
        let (hub, _task_result_rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: format!(
                    r#"{{"type":"execute_command","command":{{"type":"browser","action":"open_system","url":"https://www.baidu.com"}},"workspace_path":"{}"}}"#,
                    workspace_root
                ),
            })),
            None,
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
                MessageContent::Text("帮我访问百度".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-browser-baidu".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
                        assert_eq!(url, "https://www.baidu.com");
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
    }

    #[test]
    fn test_resolve_dingtalk_reply_target_prefers_session_webhook() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-webhook".to_string(),
            source_message_id: None,
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
    fn test_resolve_dingtalk_reply_target_prefers_session_webhook_without_expiry() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-webhook-no-expiry".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: Some("https://example.com/hook".to_string()),
            session_webhook_expired_time: None,
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
            source_message_id: None,
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
            source_message_id: None,
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
            None,
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let assignment =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        assert!(assignment.is_err());
        assert!(state.dingtalk_routes.read().await.is_empty());
        assert!(state.dingtalk_reply_handles.read().await.is_empty());

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** 你好"));
        assert!(history.contains("**Assistant:** 直接答复"));

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::AssistantFinal
                && event.content == "直接答复"
        }));

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
    async fn test_reply_dingtalk_error_reuses_noop_handle_for_session_webhook_route() {
        let runtime = create_test_runtime().await;
        let (webhook_url, requests, server_handle) = start_session_webhook_server().await;
        let channel = Arc::new(DingTalkChannel::new(
            "key".to_string(),
            "secret".to_string(),
            1,
            None,
        ));
        channel.set_access_token_for_test("test-token").await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            Some(channel),
            None,
            None,
            runtime,
        ));

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
            conversation_id: "conv-error-webhook".to_string(),
            message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: Some(webhook_url),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        reply_dingtalk_error(&state, &inbound, "boom").await.unwrap();

        let captured = requests.lock().await;
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].auth_header.as_deref(), Some("test-token"));
        assert_eq!(captured[0].body["markdown"]["title"], json!("uHorse"));
        assert_eq!(captured[0].body["markdown"]["text"], json!("执行失败：boom"));
        assert_eq!(captured[0].body["at"]["atUserIds"], json!(["staff-1"]));
        drop(captured);

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::TurnFailed
                && event.content == "boom"
        }));

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_lists_installed_skills_without_dispatching_task() {
        let (runtime, state) = create_test_runtime_with_skill(
            "echo",
            r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "import os; print(os.environ['SKILL_INPUT'])"]
 "#,
            r#"{"type":"list_installed_skills"}"#,
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
                MessageContent::Text("帮我列出已经安装的技能".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-list-skills".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** 帮我列出已经安装的技能"));
        assert!(history.contains("当前已安装 1 个 Skill"));
        assert!(history.contains("- echo：echo skill"));
        assert!(state.dingtalk_routes.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_runs_through_session_runtime_lane() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"type":"direct_reply","text":"串行答复"}"#.to_string(),
            })),
            None,
            runtime.clone(),
        ));

        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Text("phase1 测试".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-phase1".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let history = runtime
            .memory_store
            .get_context(&CoreSessionId::from_string(session_key.as_str()))
            .await
            .unwrap();
        assert!(history.contains("**User:** phase1 测试"));
        assert!(history.contains("**Assistant:** 串行答复"));
        assert!(state.dingtalk_routes.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_normalize_dingtalk_inbound_message_uses_audio_recognition_and_persists_transcript() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            None,
            runtime.clone(),
        ));
        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Audio {
                    url: "dingtalk://audio?key=audio-key-1".to_string(),
                    duration: Some(3),
                },
                1,
            ),
            session,
            conversation_id: "conv-audio".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![DingTalkInboundAttachment {
                kind: "audio".to_string(),
                key: Some("audio-key-1".to_string()),
                file_name: Some("voice.mp3".to_string()),
                download_code: Some("download-code".to_string()),
                recognition: Some("帮我总结这段语音".to_string()),
                caption: None,
            }],
        };

        let normalized = normalize_dingtalk_inbound_message(&state, &inbound, &session_key)
            .await
            .unwrap();
        match normalized {
            NormalizedDingTalkInbound::ContinueAsText {
                text,
                consumed_pending_attachments,
            } => {
                assert_eq!(text, "帮我总结这段语音");
                assert!(consumed_pending_attachments.is_empty());
            }
            other => panic!("unexpected normalized result: {:?}", other),
        }

        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session_state
                .metadata
                .get(DINGTALK_LAST_AUDIO_TRANSCRIPT_KEY)
                .map(String::as_str),
            Some("帮我总结这段语音")
        );
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_stashes_file_attachment_and_replies_via_webhook() {
        let runtime = create_test_runtime().await;
        let (webhook_url, requests, server_handle) = start_session_webhook_server().await;
        let channel = Arc::new(DingTalkChannel::new(
            "key".to_string(),
            "secret".to_string(),
            1,
            None,
        ));
        channel.set_access_token_for_test("test-token").await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            Some(channel),
            None,
            None,
            runtime.clone(),
        ));

        let session = uhorse_core::Session::new(
            uhorse_core::ChannelType::DingTalk,
            "fallback-user".to_string(),
        );
        let inbound = DingTalkInboundMessage {
            message: uhorse_core::Message::new(
                session.id.clone(),
                uhorse_core::MessageRole::User,
                MessageContent::Structured(json!({
                    "kind": "dingtalk_file",
                    "file_key": "file-key-1",
                    "file_name": "spec.pdf"
                })),
                1,
            ),
            session,
            conversation_id: "conv-file".to_string(),
            message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: Some(webhook_url),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
            robot_code: Some("robot-1".to_string()),
            attachments: vec![DingTalkInboundAttachment {
                kind: "file".to_string(),
                key: Some("file-key-1".to_string()),
                file_name: Some("spec.pdf".to_string()),
                download_code: Some("download-code".to_string()),
                recognition: None,
                caption: None,
            }],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let captured = requests.lock().await;
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].auth_header.as_deref(), Some("test-token"));
        assert_eq!(
            captured[0].body["markdown"]["text"],
            json!(DINGTALK_ATTACHMENT_WAITING_REPLY_TEXT)
        );
        drop(captured);

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let scope = runtime.agent_manager.get_scope("main").unwrap();
        let session_state = scope
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        let attachments = read_pending_dingtalk_attachments(&session_state.metadata);
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].kind, "file");
        assert_eq!(attachments[0].summary, "文件（spec.pdf）");

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_submit_dingtalk_task_merges_pending_attachment_context_into_follow_up_text() {
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
            None,
            runtime.clone(),
        ));
        let node_id = uhorse_protocol::NodeId::from_string("node-follow-up");
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
        let mut session_state = SessionState::new(session_key.as_str());
        session_state.metadata.insert(
            DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY.to_string(),
            serde_json::to_string(&vec![PendingDingTalkAttachment {
                kind: "file".to_string(),
                summary: "文件（spec.pdf）".to_string(),
                file_name: None,
                download_code: None,
            }])
            .unwrap(),
        );
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
                MessageContent::Text("请检查这个文件".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-follow-up".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

        let assignment = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let context = match assignment {
            HubToNode::TaskAssignment { context, .. } => context,
            other => panic!("unexpected message: {:?}", other),
        };
        assert_eq!(
            context.intent.as_deref(),
            Some("用户刚刚发送了以下附件上下文：\n- 文件（spec.pdf）\n\n用户本条补充说明：\n请检查这个文件")
        );

        let persisted = runtime
            .agent_manager
            .get_scope("main")
            .unwrap()
            .load_session_state(&session_key.as_str())
            .await
            .unwrap()
            .unwrap();
        assert!(!persisted
            .metadata
            .contains_key(DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY));
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
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        submit_dingtalk_task(&state, inbound, None).await.unwrap();

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
    async fn test_install_runtime_skill_api_installs_and_refreshes_registry() {
        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        let skill_toml = r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "print('installed')"]
"#;
        let archive = build_test_skill_archive("installed-skill", skill_toml);
        let (download_url, server_handle) = start_test_archive_server(archive).await;
        let app = create_router((*state).clone());

        let (status, body) = post_json(
            app,
            "/api/v1/skills/install",
            &json!({
                "source_type": "skillhub",
                "package": "installed-skill",
                "download_url": download_url
            }),
        )
        .await;

        server_handle.abort();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["skill_name"], json!("installed-skill"));
        assert_eq!(body["data"]["target_layer"], json!("global"));

        let skills = state.agent_runtime.skills.read().await;
        let entry = skills.get_any_entry("installed-skill").unwrap();
        assert_eq!(entry.source_layer, "global");
        assert!(entry.source_scope.is_none());
    }

    #[tokio::test]
    async fn test_refresh_runtime_skill_api_reloads_new_files() {
        let (workspace, state) = create_skill_install_test_state(vec![]).await;
        let app = create_router((*state).clone());

        write_test_skill(
            &workspace.path().join("agent-runtime"),
            "manual-refresh-skill",
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "print('manual')"]
"#,
        )
        .await;

        let (status, body) = post_json(app, "/api/v1/skills/refresh", &json!({})).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["skill_count"], json!(1));
        assert!(state
            .agent_runtime
            .skills
            .read()
            .await
            .get_any_entry("manual-refresh-skill")
            .is_some());
    }

    #[tokio::test]
    async fn test_actor_can_install_skill_allows_matching_dingtalk_user() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;

        let allowed = actor_can_install_skill(
            state.as_ref(),
            &SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
        );

        assert!(allowed);
    }

    #[tokio::test]
    async fn test_actor_can_install_skill_rejects_corp_mismatch() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;

        let allowed = actor_can_install_skill(
            state.as_ref(),
            &SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-2".to_string()),
            },
        );

        assert!(!allowed);
    }

    #[test]
    fn test_resolve_dingtalk_skill_install_request_parses_command() {
        let request = resolve_dingtalk_skill_install_request(
            "安装技能 demo-skill https://127.0.0.1/skill.tar.gz v1.2.3",
        )
        .unwrap();

        assert!(matches!(request.source_type, SkillInstallSourceType::Skillhub));
        assert_eq!(request.package, "demo-skill");
        assert_eq!(request.download_url, "https://127.0.0.1/skill.tar.gz");
        assert_eq!(request.version.as_deref(), Some("v1.2.3"));
        assert!(matches!(request.target_layer, SkillInstallTargetLayer::Global));
        assert!(request.target_scope.is_none());
    }

    #[test]
    fn test_parse_dingtalk_skill_install_search_query_extracts_skill_name() {
        assert_eq!(
            parse_dingtalk_skill_install_search_query("帮我安装 Agent Browser 技能").as_deref(),
            Some("Agent Browser")
        );
        assert_eq!(
            parse_dingtalk_skill_install_search_query("please install agent browser skill")
                .as_deref(),
            Some("agent browser")
        );
        assert_eq!(
            parse_dingtalk_skill_install_search_query("Agent Browser 技能").as_deref(),
            Some("Agent Browser")
        );
        assert_eq!(
            parse_dingtalk_skill_install_search_query("agent browser skill").as_deref(),
            Some("agent browser")
        );
        assert_eq!(
            parse_dingtalk_skill_install_search_query("Browser Use").as_deref(),
            Some("Browser Use")
        );
        assert!(parse_dingtalk_skill_install_search_query("帮我看看 Agent Browser 技能").is_none());
        assert!(parse_dingtalk_skill_install_search_query("帮我列出已经安装的技能").is_none());
    }

    #[test]
    fn test_infer_skillhub_slug_from_query_normalizes_words() {
        assert_eq!(
            infer_skillhub_slug_from_query("Agent Browser").as_deref(),
            Some("agent-browser")
        );
        assert_eq!(
            infer_skillhub_slug_from_query("  agent_browser  ").as_deref(),
            Some("agent-browser")
        );
        assert!(infer_skillhub_slug_from_query("！！！").is_none());
    }

    #[test]
    fn test_resolve_dingtalk_reply_target_returns_none_without_webhook_group_or_user() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-missing".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: None,
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(resolve_dingtalk_reply_target(&route), None);
    }

    #[test]
    fn test_resolve_dingtalk_ai_card_target_accepts_group_with_robot_code() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-group".to_string(),
            source_message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(
            resolve_dingtalk_ai_card_target(&route),
            Some(DingTalkAiCardTarget::ImGroup {
                conversation_id: "conv-group".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_dingtalk_ai_card_target_rejects_robot_chat_route() {
        let route = DingTalkReplyRoute {
            conversation_id: "conv-session".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: Some("https://example.com/hook".to_string()),
            session_webhook_expired_time: Some(chrono::Utc::now().timestamp_millis() + 60_000),
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(resolve_dingtalk_ai_card_target(&route), None);
    }

    #[test]
    fn test_resolve_dingtalk_ai_card_target_rejects_missing_robot_code_and_private_chat() {
        let missing_robot_route = DingTalkReplyRoute {
            conversation_id: "conv-group".to_string(),
            source_message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: None,
        };
        let private_route = DingTalkReplyRoute {
            conversation_id: "conv-direct".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: None,
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        assert_eq!(resolve_dingtalk_ai_card_target(&missing_robot_route), None);
        assert_eq!(resolve_dingtalk_ai_card_target(&private_route), None);
    }

    #[tokio::test]
    async fn test_send_dingtalk_reply_returns_error_without_resolved_target() {
        let channel = DingTalkChannel::new("key".to_string(), "secret".to_string(), 1, None);
        let route = DingTalkReplyRoute {
            conversation_id: "conv-missing".to_string(),
            source_message_id: None,
            conversation_type: Some("1".to_string()),
            sender_user_id: None,
            sender_staff_id: Some("staff-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
        };

        let error = send_dingtalk_reply(&channel, &route, "hello")
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "No DingTalk reply target could be resolved");
    }

    #[test]
    fn test_format_dingtalk_reply_text_collapses_extra_blank_lines() {
        let formatted = format_dingtalk_reply_text("第一行\n\n\n第二行\n\n");
        assert_eq!(formatted, "第一行\n\n第二行");
    }

    #[test]
    fn test_format_dingtalk_reply_text_strips_code_fence_markers() {
        let formatted = format_dingtalk_reply_text("```\nlet a = 1;\n```");
        assert_eq!(formatted, "let a = 1;");
    }

    #[test]
    fn test_format_dingtalk_reply_text_normalizes_markdown_headers() {
        let formatted = format_dingtalk_reply_text("# 标题\n## 小节\n正文");
        assert_eq!(formatted, "标题\n小节\n正文");
    }

    #[test]
    fn test_send_dingtalk_reply_applies_dingtalk_text_formatting_before_send() {
        let formatted = format_dingtalk_reply_text("# 标题\n\n```\n内容\n```");
        assert_eq!(formatted, "标题\n\n内容");
    }

    #[test]
    fn test_should_send_dingtalk_immediate_ack_for_direct_reply_and_local_skill_steps() {
        assert!(should_send_dingtalk_immediate_ack(&PlannedTurnStep::Finalize {
            text: "done".to_string(),
        }));
        assert!(should_send_dingtalk_immediate_ack(&PlannedTurnStep::ExecuteSkill {
            skill_name: "echo".to_string(),
            input: "hello".to_string(),
        }));
        assert!(should_send_dingtalk_immediate_ack(&PlannedTurnStep::ListInstalledSkills));
        assert!(should_send_dingtalk_immediate_ack(&PlannedTurnStep::QuerySkill {
            skill_name: "echo".to_string(),
        }));
        assert!(should_send_dingtalk_immediate_ack(&PlannedTurnStep::InstallSkill {
            query: "agent-browser".to_string(),
        }));
    }

    #[test]
    fn test_should_not_send_dingtalk_immediate_ack_for_submit_task_step() {
        assert!(!should_send_dingtalk_immediate_ack(&PlannedTurnStep::SubmitTask {
            command: Command::File(FileCommand::List {
                path: ".".to_string(),
                recursive: false,
                pattern: None,
            }),
            workspace_path: Some("/tmp/workspace".to_string()),
        }));
    }

    #[test]
    fn test_should_attach_dingtalk_processing_ack_now_for_normal_text_message() {
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
            conversation_id: "conv-1".to_string(),
            message_id: Some("msg-1".to_string()),
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        assert!(should_attach_dingtalk_processing_ack_now(&inbound));
    }

    #[tokio::test]
    async fn test_enforce_dingtalk_ack_min_display_returns_quickly_when_already_elapsed() {
        let handle = DingTalkReplyHandle::LegacyTransient {
            receipt: Some(DingTalkTransientMessageReceipt::unsupported()),
            attached_at: Instant::now() - Duration::from_millis(950),
        };
        let start = Instant::now();
        enforce_dingtalk_ack_min_display(Some(&handle), Duration::from_millis(900)).await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_configured_skillhub_search_url_uses_default_when_config_and_env_missing() {
        unsafe {
            std::env::remove_var("UHORSE_SKILLHUB_SEARCH_URL");
        }
        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        assert_eq!(
            configured_skillhub_search_url(state.as_ref()),
            DEFAULT_SKILLHUB_SEARCH_URL
        );
    }

    #[tokio::test]
    async fn test_configured_skillhub_search_url_prefers_config_over_env() {
        unsafe {
            std::env::set_var("UHORSE_SKILLHUB_SEARCH_URL", "https://env.example.com/api/v1/search");
        }
        let (_workspace, state) = create_skill_install_test_state_with_skillhub(
            vec![],
            Some("https://config.example.com/api/v1/search".to_string()),
            None,
        )
        .await;
        assert_eq!(
            configured_skillhub_search_url(state.as_ref()),
            "https://config.example.com/api/v1/search"
        );
        unsafe {
            std::env::remove_var("UHORSE_SKILLHUB_SEARCH_URL");
        }
    }

    #[tokio::test]
    async fn test_build_skillhub_download_url_uses_configured_template() {
        let (_workspace, state) = create_skill_install_test_state_with_skillhub(
            vec![],
            None,
            Some("https://skillhub.example.com/api/v1/download?slug={slug}".to_string()),
        )
        .await;
        assert_eq!(
            build_skillhub_download_url(state.as_ref(), "agent-browser"),
            "https://skillhub.example.com/api/v1/download?slug=agent-browser"
        );
    }

    #[test]
    fn test_is_allowed_skillhub_download_url_only_accepts_official_hosts() {
        assert!(is_allowed_skillhub_download_url(
            "https://api.skillhub.tencent.com/api/v1/download?slug=agent-browser"
        ));
        assert!(is_allowed_skillhub_download_url(
            "https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com/skills/agent-browser.zip"
        ));
        assert!(!is_allowed_skillhub_download_url(
            "https://example.com/skills/agent-browser.zip"
        ));
    }

    #[tokio::test]
    async fn test_search_skillhub_skill_uses_search_api_result() {
        let (search_url, server_handle) = start_skillhub_search_server(
            r#"{"code":0,"data":{"skills":[{"slug":"agent-browser","title":"Agent Browser","version":"1.2.3"}]},"message":"ok"}"#,
        )
        .await;
        unsafe {
            std::env::set_var("UHORSE_SKILLHUB_SEARCH_URL", &search_url);
        }

        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        let result = search_skillhub_skill(state.as_ref(), "Agent Browser").await.unwrap();

        unsafe {
            std::env::remove_var("UHORSE_SKILLHUB_SEARCH_URL");
        }
        server_handle.abort();

        let entry = result.unwrap();
        assert_eq!(entry.slug, "agent-browser");
        assert_eq!(entry.name, "Agent Browser");
        assert_eq!(entry.version.as_deref(), Some("1.2.3"));
    }

    #[tokio::test]
    async fn test_resolve_dingtalk_skill_install_intent_fallbacks_to_inferred_slug_when_search_fails() {
        let (_workspace, state) = create_skill_install_test_state_with_skillhub(
            vec![],
            Some("http://127.0.0.1:9/api/v1/search".to_string()),
            None,
        )
        .await;

        let intent = resolve_dingtalk_skill_install_intent(
            state.as_ref(),
            "帮我安装 Agent Browser 技能",
            &[],
        )
        .await
        .unwrap();

        match intent {
            Some(DingtalkSkillInstallIntent::NaturalLanguage(entry)) => {
                assert_eq!(entry.slug, "agent-browser");
                assert_eq!(entry.name, "Agent Browser");
                assert!(entry.version.is_none());
            }
            other => panic!("unexpected intent: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_agent_browser_natural_language_install_flow_returns_chinese_hint() {
        let (search_url, search_server_handle) = start_skillhub_search_server(
            r#"{"code":0,"data":{"skills":[{"slug":"agent-browser","title":"Agent Browser","version":"1.2.3"}]},"message":"ok"}"#,
        )
        .await;
        let (_workspace, state) = create_skill_install_test_state_with_skillhub(
            vec![DingTalkSkillInstaller {
                user_id: Some("ding-user-1".to_string()),
                staff_id: None,
                corp_id: Some("ding-corp-1".to_string()),
            }],
            Some(search_url),
            None,
        )
        .await;
        let skill_toml = r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('browser')"]
 "#;
        let archive = build_test_skill_zip_archive(skill_toml);
        let (download_url, archive_server_handle) = start_test_archive_server(archive).await;

        let intent = resolve_dingtalk_skill_install_intent(
            state.as_ref(),
            "帮我安装 Agent Browser 技能",
            &[],
        )
        .await
        .unwrap();

        let entry = match intent {
            Some(DingtalkSkillInstallIntent::NaturalLanguage(entry)) => entry,
            other => panic!("unexpected intent: {:?}", other),
        };
        assert_eq!(entry.slug, "agent-browser");
        assert_eq!(entry.name, "Agent Browser");
        assert_eq!(entry.version.as_deref(), Some("1.2.3"));

        let result = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: entry.slug.clone(),
                version: entry.version.clone(),
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap();

        search_server_handle.abort();
        archive_server_handle.abort();

        assert_eq!(result.skill_name, "agent-browser");
        assert!(state
            .agent_runtime
            .skills
            .read()
            .await
            .get_any_entry("agent-browser")
            .is_some());

        let hint = build_skill_install_trigger_hint(&state, &result).await;
        let reply = format!("Skill {} 安装成功。{}", result.skill_name, hint);
        assert!(reply.contains("Skill agent-browser 安装成功"));
        assert!(reply.contains("你现在可以直接用自然语言描述需求"));
        assert!(reply.contains("例如：请打开目标网页，帮我完成点击、输入或截图，并把结果发给我"));
    }

    #[tokio::test]
    async fn test_normalize_dingtalk_inbound_message_turns_pending_zip_install_into_attachment_install_query() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            None,
            None,
            runtime.clone(),
        ));

        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let mut session_state = SessionState::new(session_key.as_str());
        session_state.metadata.insert(
            DINGTALK_PENDING_ATTACHMENT_CONTEXT_KEY.to_string(),
            serde_json::to_string(&vec![PendingDingTalkAttachment {
                kind: "file".to_string(),
                summary: "文件（agent-browser.zip）".to_string(),
                file_name: Some("agent-browser.zip".to_string()),
                download_code: Some("download-code-1".to_string()),
            }])
            .unwrap(),
        );
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
                MessageContent::Text("帮我安装这个技能".to_string()),
                1,
            ),
            session,
            conversation_id: "conv-install-follow-up".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("actual-user".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        let normalized = normalize_dingtalk_inbound_message(&state, &inbound, &session_key)
            .await
            .unwrap();
        match normalized {
            NormalizedDingTalkInbound::ContinueAsText {
                text,
                consumed_pending_attachments,
            } => {
                assert_eq!(text, "帮我安装这个技能\n\n附件技能包：agent-browser.zip");
                assert_eq!(consumed_pending_attachments.len(), 1);
                assert_eq!(
                    consumed_pending_attachments[0].download_code.as_deref(),
                    Some("download-code-1")
                );
            }
            other => panic!("unexpected normalized result: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_resolve_dingtalk_skill_install_intent_prefers_pending_zip_attachment() {
        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        let pending_attachments = vec![PendingDingTalkAttachment {
            kind: "file".to_string(),
            summary: "文件（agent-browser.zip）".to_string(),
            file_name: Some("agent-browser.zip".to_string()),
            download_code: Some("download-code-1".to_string()),
        }];

        let intent = resolve_dingtalk_skill_install_intent(
            state.as_ref(),
            "帮我安装这个技能",
            &pending_attachments,
        )
        .await
        .unwrap();

        match intent {
            Some(DingtalkSkillInstallIntent::ExplicitCommand(request)) => {
                assert!(matches!(
                    request.source_type,
                    SkillInstallSourceType::DingtalkAttachment
                ));
                assert_eq!(request.package, "agent-browser");
                assert_eq!(request.attachment_file_name.as_deref(), Some("agent-browser.zip"));
                assert_eq!(
                    request.attachment_download_code.as_deref(),
                    Some("download-code-1")
                );
            }
            other => panic!("unexpected intent: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_resolve_dingtalk_skill_install_intent_prefers_pending_zip_attachment_when_query_is_file_name() {
        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        let pending_attachments = vec![PendingDingTalkAttachment {
            kind: "file".to_string(),
            summary: "文件（audit-newsletter-expert-v1.0.0-20260412.zip）".to_string(),
            file_name: Some("audit-newsletter-expert-v1.0.0-20260412.zip".to_string()),
            download_code: Some("download-code-zip-1".to_string()),
        }];

        let intent = resolve_dingtalk_skill_install_intent(
            state.as_ref(),
            "audit-newsletter-expert-v1.0.0-20260412.zip",
            &pending_attachments,
        )
        .await
        .unwrap();

        match intent {
            Some(DingtalkSkillInstallIntent::ExplicitCommand(request)) => {
                assert!(matches!(
                    request.source_type,
                    SkillInstallSourceType::DingtalkAttachment
                ));
                assert_eq!(request.package, "audit-newsletter-expert-v1.0.0-20260412");
                assert_eq!(
                    request.attachment_file_name.as_deref(),
                    Some("audit-newsletter-expert-v1.0.0-20260412.zip")
                );
                assert_eq!(
                    request.attachment_download_code.as_deref(),
                    Some("download-code-zip-1")
                );
            }
            other => panic!("unexpected intent: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_parse_agent_decision_does_not_echo_unknown_json_as_direct_reply() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(StubLlmClient {
                response: r#"{"execute_command":{"type":"shell","command":"find / -name 'audit-newsletter-expert-v1.0.0-20260412.zip' 2>/dev/null | head -20","args":[],"cwd":null,"env":{},"timeout":120,"capture_stderr":true}}"#.to_string(),
            })),
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-json-guard");

        let decision = decide_dingtalk_action(
            &state,
            "你去读一下啊，压缩包我都发给你了",
            "main",
            &session_key,
        )
        .await
        .unwrap();

        assert!(matches!(
            decision,
            AgentDecision::DirectReply { ref text }
            if text == "我没有正确理解你的意思。请直接告诉我你希望我怎么处理刚刚上传的附件，例如：帮我安装这个技能。"
        ));
    }

    #[tokio::test]
    async fn test_unpack_skill_archive_accepts_zip_with_nested_root_dir() {
        let skill_toml = r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('nested')"]
 "#;
        let skill_md = "---\nname: audit-newsletter-expert\nversion: 1.0.0\ndescription: audit newsletter expert\nauthor: test\nparameters: []\npermissions: []\n---\n";
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file(
                "audit-newsletter-expert-v1.0.0-20260412/SKILL.md",
                options,
            )
            .unwrap();
        std::io::Write::write_all(&mut writer, skill_md.as_bytes()).unwrap();
        writer
            .start_file(
                "audit-newsletter-expert-v1.0.0-20260412/skill.toml",
                options,
            )
            .unwrap();
        std::io::Write::write_all(&mut writer, skill_toml.as_bytes()).unwrap();
        writer.finish().unwrap();

        let temp = tempdir().unwrap();
        let skill_name = unpack_skill_archive(
            &bytes.into_inner(),
            temp.path(),
            "audit-newsletter-expert-v1.0.0-20260412",
        )
        .await
        .unwrap();

        assert_eq!(skill_name, "audit-newsletter-expert-v1.0.0-20260412");
        assert!(temp.path().join(&skill_name).join("SKILL.md").exists());
        assert!(temp.path().join(&skill_name).join("skill.toml").exists());
    }

    #[tokio::test]
    async fn test_install_skill_generates_skill_toml_from_skill_yaml_python_entrypoint() {
        let (_workspace, state) = create_skill_install_test_state(vec![]).await;
        let skill_md = "---\nname: audit-newsletter-expert\nversion: 1.0.0\ndescription: audit newsletter expert\nauthor: test\nparameters: []\npermissions: []\n---\n";
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("SKILL.md", options).unwrap();
        std::io::Write::write_all(&mut writer, skill_md.as_bytes()).unwrap();
        writer.start_file("skill.yaml", options).unwrap();
        std::io::Write::write_all(&mut writer, b"name: audit-newsletter-expert\n").unwrap();
        writer.start_file("requirements.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"PyYAML>=6.0\n").unwrap();
        writer.start_file("src/main.py", options).unwrap();
        std::io::Write::write_all(&mut writer, b"import yaml\nprint('ok')\n").unwrap();
        writer.finish().unwrap();

        let (download_url, archive_server_handle) = start_test_archive_server(bytes.into_inner()).await;
        let result = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "api",
                sender_user_id: Some("user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: None,
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "audit-newsletter-skill".to_string(),
                version: Some("1.0.0".to_string()),
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap();
        archive_server_handle.abort();

        let installed_dir = build_skill_install_dir(&state.agent_runtime.runtime_root, &result);
        let generated = tokio::fs::read_to_string(installed_dir.join("skill.toml"))
            .await
            .unwrap();
        assert!(generated.contains("enabled = true"));
        assert!(generated.contains("args = [\"src/main.py\"]"));
        assert!(generated.contains(".venv/bin/python3"));
        assert!(installed_dir.join(".venv").join("bin").join("python3").exists());
        assert!(state
            .agent_runtime
            .skills
            .read()
            .await
            .get_any_entry("audit-newsletter-expert")
            .is_some());
    }

    #[tokio::test]
    async fn test_search_skillhub_skill_reports_decode_context() {
        let (search_url, server_handle) = start_skillhub_search_server("not-json").await;
        let (_workspace, state) = create_skill_install_test_state_with_skillhub(
            vec![],
            Some(search_url),
            None,
        )
        .await;

        let error = search_skillhub_skill(state.as_ref(), "Agent Browser")
            .await
            .unwrap_err()
            .to_string();

        server_handle.abort();

        assert!(error.contains("Skill 搜索响应解析失败"));
        assert!(error.contains("query=Agent Browser"));
    }

    #[tokio::test]
    async fn test_install_skill_from_request_allows_authorized_dingtalk_actor() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;
        let skill_toml = r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "print('authorized')"]
"#;
        let archive = build_test_skill_archive("ding-install-skill", skill_toml);
        let (download_url, server_handle) = start_test_archive_server(archive).await;

        let result = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "ding-install-skill".to_string(),
                version: None,
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await;

        server_handle.abort();

        let response = result.unwrap();
        assert_eq!(response.skill_name, "ding-install-skill");
        assert!(state
            .agent_runtime
            .skills
            .read()
            .await
            .get_any_entry("ding-install-skill")
            .is_some());
    }

    #[tokio::test]
    async fn test_install_skill_from_request_returns_unpack_error_for_invalid_archive() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;
        let (download_url, server_handle) = start_test_archive_server(b"not-an-archive".to_vec()).await;

        let error = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "agent-browser".to_string(),
                version: None,
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap_err()
        .to_string();

        server_handle.abort();

        assert!(error.contains("Skill 安装包格式非法"));
    }

    #[tokio::test]
    async fn test_install_skill_from_request_accepts_zip_archive_without_root_dir() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;
        let skill_toml = r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "print('zip')"]
"#;
        let archive = build_test_skill_zip_archive(skill_toml);
        let (download_url, server_handle) = start_test_archive_server(archive).await;

        let result = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "agent-browser".to_string(),
                version: None,
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await;

        server_handle.abort();

        let response = result.unwrap();
        assert_eq!(response.skill_name, "agent-browser");
        assert!(state
            .agent_runtime
            .skills
            .read()
            .await
            .get_any_entry("agent-browser")
            .is_some());
    }

    #[test]
    fn test_extract_skill_usage_hint_prefers_first_meaningful_body_line() {
        let skill_md = r#"---
name: Agent Browser
description: browser automation
author: test
version: 1.0.0
parameters: []
permissions: []
---

# Browser Automation with agent-browser

- 打开网页并执行点击、输入、抓取页面内容
- 适合自动化网页操作
"#;

        let hint = extract_skill_usage_hint(skill_md).unwrap();
        assert_eq!(hint, "打开网页并执行点击、输入、抓取页面内容");
    }

    #[tokio::test]
    async fn test_build_skill_install_trigger_hint_uses_manifest_description() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;
        let skill_toml = r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('zip')"]
 "#;
        let archive = build_test_skill_zip_archive(skill_toml);
        let (download_url, server_handle) = start_test_archive_server(archive).await;

        install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "agent-browser".to_string(),
                version: None,
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap();

        server_handle.abort();

        let hint = build_skill_install_trigger_hint(
            &state,
            &SkillInstallResponse {
                skill_name: "agent-browser".to_string(),
                source_type: "skillhub".to_string(),
                package: "agent-browser".to_string(),
                version: None,
                target_layer: "global".to_string(),
                target_scope: None,
            },
        )
        .await;
        assert!(hint.contains("这个 Skill 主要用于：agent browser skill"));
        assert!(hint.contains("你现在可以直接用自然语言描述需求"));
        assert!(hint.contains("例如：请打开目标网页，帮我完成点击、输入或截图，并把结果发给我"));
    }

    #[tokio::test]
    async fn test_build_skill_install_trigger_hint_prefers_llm_simplified_chinese_summary() {
        let (_runtime, state) = create_test_runtime_with_skill(
            "browser-use",
            r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('ok')"]
 "#,
            r#"{"description_zh":"用于自动化操作网页并提取页面信息","example_zh":"请打开目标网页，帮我填写表单并截图发给我"}"#,
        )
        .await;

        let hint = build_skill_install_trigger_hint(
            &state,
            &SkillInstallResponse {
                skill_name: "browser-use".to_string(),
                source_type: "skillhub".to_string(),
                package: "browser-use".to_string(),
                version: None,
                target_layer: "global".to_string(),
                target_scope: None,
            },
        )
        .await;
        assert!(hint.contains("这个 Skill 主要用于：用于自动化操作网页并提取页面信息"));
        assert!(hint.contains("例如：请打开目标网页，帮我填写表单并截图发给我"));
    }

    #[tokio::test]
    async fn test_build_skill_install_trigger_hint_falls_back_to_chinese_example_when_llm_fails() {
        let runtime = create_test_runtime_with_skill_only(
            "browser-use",
            r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('ok')"]
 "#,
        )
        .await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
            Some(Arc::new(FailingLlmClient)),
            None,
            runtime,
        ));

        let hint = build_skill_install_trigger_hint(
            &state,
            &SkillInstallResponse {
                skill_name: "browser-use".to_string(),
                source_type: "skillhub".to_string(),
                package: "browser-use".to_string(),
                version: None,
                target_layer: "global".to_string(),
                target_scope: None,
            },
        )
        .await;
        assert!(hint.contains("你现在可以直接用自然语言描述需求"));
        assert!(hint.contains("例如：请打开目标网页，帮我完成点击、输入或截图，并把结果发给我"));
    }

    #[tokio::test]
    async fn test_build_skill_install_trigger_hint_reads_installed_skill_md_body() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;
        let skill_md = r#"---
name: browser-use
version: 1.0.0
description: Automates browser interactions for web testing, form filling, screenshots, and data extraction.
author: test
parameters: []
permissions: []
---

# Browser Automation with browser-use CLI

The `browser-use` command provides fast, persistent browser automation.
"#;
        let skill_toml = r#"enabled = true
 timeout = 5
 executable = "python3"
 args = ["-c", "print('zip')"]
 "#;
        let archive = build_test_skill_zip_archive_with_skill_md(skill_md, skill_toml);
        let (download_url, server_handle) = start_test_archive_server(archive).await;

        let result = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-1".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "browser-use".to_string(),
                version: None,
                download_url,
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap();

        server_handle.abort();

        let hint = build_skill_install_trigger_hint(&state, &result).await;
        assert!(hint.contains("例如：请打开目标网页，帮我完成点击、输入或截图，并把结果发给我"));
    }

    #[tokio::test]
    async fn test_install_skill_from_request_rejects_unauthorized_dingtalk_actor() {
        let (_workspace, state) = create_skill_install_test_state(vec![DingTalkSkillInstaller {
            user_id: Some("ding-user-1".to_string()),
            staff_id: None,
            corp_id: Some("ding-corp-1".to_string()),
        }])
        .await;

        let error = install_skill_from_request(
            &state,
            SkillInstallActor {
                channel: "dingtalk",
                sender_user_id: Some("ding-user-2".to_string()),
                sender_staff_id: None,
                sender_corp_id: Some("ding-corp-1".to_string()),
            },
            SkillInstallRequest {
                source_type: SkillInstallSourceType::Skillhub,
                package: "ding-install-skill".to_string(),
                version: None,
                download_url: "http://127.0.0.1:9/skill.tar.gz".to_string(),
                target_layer: SkillInstallTargetLayer::Global,
                target_scope: None,
                attachment_download_code: None,
                attachment_file_name: None,
            },
        )
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "当前账号没有安装 Skill 的权限。");
    }

    #[tokio::test]
    async fn test_list_runtime_sessions_and_messages_return_runtime_state() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let state = Arc::new(WebState::new_with_runtime(
            Arc::new(hub),
            None,
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
    async fn test_runtime_diagnostics_and_session_detail_include_mailbox_state() {
        let runtime = create_test_runtime().await;
        let (hub, _rx) = Hub::new(HubConfig::default());
        let hub = Arc::new(hub);
        let state = Arc::new(WebState::new_with_runtime(
            hub.clone(),
            None,
            None,
            None,
            runtime,
        ));
        let session_key = SessionKey::new("dingtalk", "user-mailbox");
        let turn_id = hub
            .session_runtime()
            .start_turn(&session_key.as_str(), "执行任务")
            .await;
        let tool_call_id = hub
            .session_runtime()
            .begin_tool_call(&session_key.as_str(), "hub_task")
            .await
            .unwrap();
        let task_id = TaskId::from_string("task-runtime-mailbox");
        hub.session_runtime()
            .bind_task_to_turn(
                task_id,
                TaskContinuationBinding {
                    session_key: session_key.as_str().to_string(),
                    turn_id,
                    tool_call_id,
                    agent_id: "main".to_string(),
                    route: DingTalkReplyRoute {
                        conversation_id: "conv-mailbox".to_string(),
                        source_message_id: None,
                        conversation_type: Some("2".to_string()),
                        sender_user_id: Some("user-mailbox".to_string()),
                        sender_staff_id: Some("staff-mailbox".to_string()),
                        session_webhook: None,
                        session_webhook_expired_time: None,
                        robot_code: None,
                    },
                },
            )
            .await;
        hub.session_runtime()
            .mark_waiting_for_approval(&session_key.as_str())
            .await;

        persist_session_state(
            &state,
            &session_key,
            "main",
            "conv-mailbox",
            Some("user-mailbox"),
            Some("staff-mailbox"),
            None,
            None,
            None,
        )
        .await;

        let app = create_router((*state).clone());
        let (diag_status, diag_body) = get_json(app.clone(), "/api/runtime/diagnostics").await;
        assert_eq!(diag_status, StatusCode::OK);
        assert_eq!(diag_body["success"], json!(true));
        assert_eq!(diag_body["data"]["waiting_for_approval_turns"], json!(1));
        assert_eq!(diag_body["data"]["active_task_bindings"], json!(1));
        assert_eq!(diag_body["data"]["session_mailboxes"][0]["session_key"], json!(session_key.as_str()));

        let (detail_status, detail_body) = get_json(
            app,
            &format!("/api/v1/sessions/{}", session_key.as_str()),
        )
        .await;
        assert_eq!(detail_status, StatusCode::OK);
        assert_eq!(
            detail_body["data"]["runtime_mailbox"]["turn"]["status"],
            json!("WaitingForApproval")
        );
        assert_eq!(
            detail_body["data"]["runtime_mailbox"]["active_task_binding_count"],
            json!(1)
        );
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
    async fn test_approval_decision_records_audit_events() {
        let logger = AuditLogger::with_in_memory_storage(256).install_global();
        logger.clear_recorded_events().await;

        let (state, node_id, _rx) = create_security_test_state().await;
        let approved = create_pending_approval(&state, &node_id, "request-audit-approve").await;
        let rejected = create_pending_approval(&state, &node_id, "request-audit-reject").await;

        let (approved_status, Json(approved_response)) = approve_approval(
            State(state.clone()),
            Path(approved.id.clone()),
            Json(ApprovalDecisionPayload {
                responder: "admin".to_string(),
                reason: Some("looks good".to_string()),
            }),
        )
        .await;
        assert_eq!(approved_status, StatusCode::OK);
        assert_eq!(
            approved_response.data.unwrap().status,
            uhorse_security::ApprovalStatus::Approved
        );

        let (rejected_status, Json(rejected_response)) = reject_approval(
            State(state.clone()),
            Path(rejected.id.clone()),
            Json(ApprovalDecisionPayload {
                responder: "auditor".to_string(),
                reason: Some("too risky".to_string()),
            }),
        )
        .await;
        assert_eq!(rejected_status, StatusCode::OK);
        assert_eq!(
            rejected_response.data.unwrap().status,
            uhorse_security::ApprovalStatus::Rejected
        );

        let events = logger.recorded_events().await;
        let approved_event = events
            .iter()
            .find(|event| {
                event.action == "approval_approved"
                    && event.target.as_deref() == Some(approved.id.as_str())
            })
            .expect("missing approval_approved audit event");
        assert_eq!(approved_event.actor.as_deref(), Some("admin"));
        assert_eq!(approved_event.target.as_deref(), Some(approved.id.as_str()));
        assert_eq!(approved_event.session_id, None);
        assert_eq!(
            approved_event
                .details
                .as_ref()
                .and_then(|value| value.get("task_id"))
                .and_then(|value| value.as_str()),
            Some("task-approval-web")
        );
        assert_eq!(
            approved_event
                .details
                .as_ref()
                .and_then(|value| value.get("node_id"))
                .and_then(|value| value.as_str()),
            Some(node_id.as_str())
        );
        assert_eq!(
            approved_event
                .details
                .as_ref()
                .and_then(|value| value.get("reason"))
                .and_then(|value| value.as_str()),
            Some("looks good")
        );

        let rejected_event = events
            .iter()
            .find(|event| {
                event.action == "approval_rejected"
                    && event.target.as_deref() == Some(rejected.id.as_str())
            })
            .expect("missing approval_rejected audit event");
        assert_eq!(rejected_event.actor.as_deref(), Some("auditor"));
        assert_eq!(rejected_event.target.as_deref(), Some(rejected.id.as_str()));
        assert_eq!(
            rejected_event
                .details
                .as_ref()
                .and_then(|value| value.get("reason"))
                .and_then(|value| value.as_str()),
            Some("too risky")
        );
    }

    #[tokio::test]
    async fn test_approve_approval_appends_transcript_event_for_bound_turn() {
        let (state, node_id, mut rx) = create_security_test_state().await;
        let session_key = SessionKey::with_team("dingtalk", "actual-user", "corp-1");
        let turn_id = state
            .hub
            .session_runtime()
            .start_turn(&session_key.as_str(), "等待审批")
            .await;
        let tool_call_id = state
            .hub
            .session_runtime()
            .begin_tool_call(&session_key.as_str(), "hub_task")
            .await
            .unwrap();
        let task_id = TaskId::from_string("task-approval-bound");
        state
            .hub
            .session_runtime()
            .bind_task_to_turn(
                task_id.clone(),
                TaskContinuationBinding {
                    session_key: session_key.as_str().to_string(),
                    turn_id,
                    tool_call_id,
                    agent_id: "main".to_string(),
                    route: DingTalkReplyRoute {
                        conversation_id: "conv-approval-bound".to_string(),
                        source_message_id: None,
                        conversation_type: Some("2".to_string()),
                        sender_user_id: Some("actual-user".to_string()),
                        sender_staff_id: Some("staff-1".to_string()),
                        session_webhook: None,
                        session_webhook_expired_time: None,
                        robot_code: Some("robot-1".to_string()),
                    },
                },
            )
            .await;
        let approval = create_pending_approval_with_task(
            &state,
            &node_id,
            "request-approve-bound",
            task_id.as_str(),
        )
        .await;

        let (status, Json(response)) = approve_approval(
            State(state.clone()),
            Path(approval.id.clone()),
            Json(ApprovalDecisionPayload {
                responder: "admin".to_string(),
                reason: Some("looks good".to_string()),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            response.data.unwrap().status,
            uhorse_security::ApprovalStatus::Approved
        );

        let _message = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        let transcript = state
            .hub
            .session_runtime()
            .transcript(&session_key.as_str())
            .await
            .unwrap();
        assert!(transcript.events.iter().any(|event| {
            event.kind == crate::session_runtime::TranscriptEventKind::ApprovalApproved
                && event.content.contains("responder=admin")
        }));
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
    fn test_resolve_pairing_command_accepts_plain_and_prefixed_code() {
        assert_eq!(resolve_pairing_command("123456"), Some("123456"));
        assert_eq!(resolve_pairing_command("绑定码 123456"), Some("123456"));
        assert_eq!(resolve_pairing_command("pair 123456"), Some("123456"));
        assert_eq!(resolve_pairing_command("bind 123456"), Some("123456"));
        assert_eq!(resolve_pairing_command("hello"), None);
    }

    #[tokio::test]
    async fn test_process_dingtalk_pairing_command_sets_runtime_binding() {
        let (_app, state, _hub, node_id, _rx, _workspace) =
            create_router_test_state_with_registered_node().await;
        let pairing_manager = state.pairing_manager.as_ref().unwrap().clone();
        let request = pairing_manager
            .initiate_pairing(
                uhorse_core::DeviceId::from_string(node_id.as_str()),
                "Desktop Node".to_string(),
                "desktop".to_string(),
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
                MessageContent::Text(format!("绑定码 {}", request.pairing_code)),
                1,
            ),
            session,
            conversation_id: "conv-pairing".to_string(),
            message_id: None,
            conversation_type: Some("2".to_string()),
            sender_user_id: Some("ding-user-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            sender_corp_id: Some("corp-1".to_string()),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            attachments: vec![],
        };

        let reply_text = process_dingtalk_pairing_command(&state, &inbound)
            .await
            .unwrap()
            .unwrap();

        assert!(reply_text.contains("绑定成功"));
        assert_eq!(
            state
                .hub
                .notification_bindings()
                .get_user_id(node_id.as_str())
                .await,
            Some("ding-user-1".to_string())
        );

        let status = pairing_manager
            .get_pairing_status(&uhorse_core::DeviceId::from_string(node_id.as_str()))
            .await
            .unwrap();
        assert_eq!(status, uhorse_security::PairingStatus::Paired);
    }

    #[tokio::test]
    async fn test_account_pairing_start_requires_authorization() {
        let (app, _state, _hub, node_id, _rx, _workspace) =
            create_router_test_state_with_registered_node().await;

        let (status, body) = post_json(
            app,
            "/api/account/pairing/start",
            &json!({
                "node_id": node_id.as_str(),
                "node_name": "Desktop Node",
                "device_type": "desktop"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["success"], json!(false));
        assert_eq!(body["error"], json!("Security manager not configured"));
    }

    #[tokio::test]
    async fn test_account_pairing_start_and_status_api_returns_pending_request() {
        let (app, state, node_id, _rx) = create_router_test_state_with_security().await;
        let auth_token = issue_test_node_token(&state, &node_id).await;

        let (status, body) = post_json_with_auth(
            app,
            "/api/account/pairing/start",
            &json!({
                "node_id": node_id.as_str(),
                "node_name": "Desktop Node",
                "device_type": "desktop"
            }),
            Some(&auth_token),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["node_id"], json!(node_id.as_str()));
        assert_eq!(body["data"]["node_name"], json!("Desktop Node"));
        assert_eq!(body["data"]["device_type"], json!("desktop"));
        assert_eq!(body["data"]["status"], json!("pending"));
        assert_eq!(body["data"]["bound_user_id"], serde_json::Value::Null);
        let request_id = body["data"]["request_id"].as_str().unwrap().to_string();
        let pairing_code = body["data"]["pairing_code"].as_str().unwrap().to_string();
        assert_eq!(pairing_code.len(), 6);

        let (status, status_body) = get_json_with_auth(
            create_router((*state.as_ref()).clone()),
            &format!("/api/account/status/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(status_body["success"], json!(true));
        assert_eq!(status_body["data"]["node_id"], json!(node_id.as_str()));
        assert_eq!(status_body["data"]["pairing_enabled"], json!(true));
        assert_eq!(
            status_body["data"]["bound_user_id"],
            serde_json::Value::Null
        );
        assert_eq!(
            status_body["data"]["pairing"]["request_id"],
            json!(request_id)
        );
        assert_eq!(
            status_body["data"]["pairing"]["pairing_code"],
            json!(pairing_code)
        );
        assert_eq!(status_body["data"]["pairing"]["status"], json!("pending"));
    }

    #[tokio::test]
    async fn test_account_status_rejects_token_node_id_mismatch() {
        let (app, state, node_id, _rx) = create_router_test_state_with_security().await;
        let other_node_id = uhorse_protocol::NodeId::from_string("node-other-web");
        let auth_token = issue_test_node_token(&state, &other_node_id).await;

        let (status, body) = get_json_with_auth(
            app,
            &format!("/api/account/status/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["success"], json!(false));
        assert_eq!(
            body["error"],
            json!("Token node_id does not match requested node_id")
        );
    }

    #[tokio::test]
    async fn test_account_pairing_cancel_api_marks_request_cancelled() {
        let (app, state, node_id, _rx) = create_router_test_state_with_security().await;
        let auth_token = issue_test_node_token(&state, &node_id).await;

        let (_status, body) = post_json_with_auth(
            app,
            "/api/account/pairing/start",
            &json!({
                "node_id": node_id.as_str()
            }),
            Some(&auth_token),
        )
        .await;
        let request_id = body["data"]["request_id"].as_str().unwrap().to_string();

        let (status, cancel_body) = post_json_with_auth(
            create_router((*state.as_ref()).clone()),
            "/api/account/pairing/cancel",
            &json!({
                "request_id": request_id
            }),
            Some(&auth_token),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cancel_body["success"], json!(true));
        assert_eq!(cancel_body["data"], json!("Pairing cancelled"));

        let (status, status_body) = get_json_with_auth(
            create_router((*state.as_ref()).clone()),
            &format!("/api/account/status/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(status_body["data"]["pairing"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_account_binding_delete_api_unbinds_runtime_binding() {
        let (app, state, node_id, _rx) = create_router_test_state_with_security().await;
        let auth_token = issue_test_node_token(&state, &node_id).await;
        let pairing_manager = state.pairing_manager.as_ref().unwrap().clone();
        let request = pairing_manager
            .initiate_pairing(
                uhorse_core::DeviceId::from_string(node_id.as_str()),
                "Desktop Node".to_string(),
                "desktop".to_string(),
            )
            .await
            .unwrap();
        pairing_manager
            .confirm_pairing(&request.pairing_code, "ding-user-1".to_string())
            .await
            .unwrap();
        state
            .hub
            .notification_bindings()
            .set_binding(node_id.as_str(), "ding-user-1")
            .await;

        let (status, body) = delete_json_with_auth(
            app,
            &format!("/api/account/binding/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"], json!("Binding removed"));
        assert_eq!(
            state
                .hub
                .notification_bindings()
                .get_user_id(node_id.as_str())
                .await,
            None
        );

        let (status, status_body) = get_json_with_auth(
            create_router((*state.as_ref()).clone()),
            &format!("/api/account/status/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            status_body["data"]["bound_user_id"],
            serde_json::Value::Null
        );
        assert_eq!(status_body["data"]["pairing"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_get_account_status_api_shows_bound_user_after_pairing_confirmation() {
        let (_app, state, node_id, _rx) = create_router_test_state_with_security().await;
        let auth_token = issue_test_node_token(&state, &node_id).await;
        let pairing_manager = state.pairing_manager.as_ref().unwrap().clone();
        let request = pairing_manager
            .initiate_pairing(
                uhorse_core::DeviceId::from_string(node_id.as_str()),
                "Desktop Node".to_string(),
                "desktop".to_string(),
            )
            .await
            .unwrap();
        pairing_manager
            .confirm_pairing(&request.pairing_code, "ding-user-1".to_string())
            .await
            .unwrap();
        state
            .hub
            .notification_bindings()
            .set_binding(node_id.as_str(), "ding-user-1")
            .await;

        let (status, body) = get_json_with_auth(
            create_router((*state.as_ref()).clone()),
            &format!("/api/account/status/{}", node_id.as_str()),
            Some(&auth_token),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["pairing_enabled"], json!(true));
        assert_eq!(body["data"]["bound_user_id"], json!("ding-user-1"));
        assert_eq!(body["data"]["pairing"], serde_json::Value::Null);
    }

    #[test]
    fn test_pairing_request_response_serializes_paired_user() {
        let mut request = PairingRequest::new(
            uhorse_core::DeviceId::from_string("node-desktop-1"),
            "Desktop Node".to_string(),
            "desktop".to_string(),
        );
        request.confirm("ding-user-1".to_string());

        let response = pairing_request_response(request);

        assert_eq!(response.node_id, "node-desktop-1");
        assert_eq!(response.status, "paired");
        assert_eq!(response.bound_user_id.as_deref(), Some("ding-user-1"));
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
