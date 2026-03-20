//! Web 管理界面
//!
//! 提供 Hub 的 HTTP 管理接口

pub mod ws;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use uhorse_channel::{dingtalk::DingTalkEvent, DingTalkChannel, DingTalkInboundMessage};
use uhorse_core::{Channel, MessageContent};
use uhorse_llm::{ChatMessage, LLMClient};
use uhorse_protocol::{Command, CommandOutput, FileCommand, TaskContext, TaskId, UserId};

use crate::{task_scheduler::{CompletedTask, TaskResult}, Hub, HubStats};
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

/// Web 服务器状态
#[derive(Clone)]
pub struct WebState {
    /// Hub 引用
    pub hub: Arc<Hub>,
    /// DingTalk 通道
    pub dingtalk_channel: Option<Arc<DingTalkChannel>>,
    /// LLM 客户端
    pub llm_client: Option<Arc<dyn LLMClient>>,
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
        Self {
            hub,
            dingtalk_channel,
            llm_client,
            dingtalk_routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
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
    let mut router = Router::new()
        // 页面路由
        .route("/", get(index_page))
        .route("/dashboard", get(dashboard_page))
        // WebSocket 路由 (Node 连接)
        .route("/ws", get(ws_handler))
        // DingTalk 回调路由
        .route("/api/v1/channels/dingtalk/webhook", post(dingtalk_webhook))
        .route("/api/v1/channels/dingtalk/webhook", get(dingtalk_webhook_verify))
        // API 路由
        .route("/api/stats", get(get_stats))
        .route("/api/nodes", get(list_nodes))
        .route("/api/nodes/:node_id", get(get_node))
        .route("/api/tasks", get(list_tasks))
        .route("/api/tasks/:task_id", get(get_task))
        .route("/api/tasks/:task_id/cancel", post(cancel_task))
        .route("/api/health", get(health_check))
        .with_state(Arc::new(state));

    // 添加 CORS
    router = router.layer(CorsLayer::permissive());

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

pub async fn submit_dingtalk_task(
    state: &Arc<WebState>,
    inbound: DingTalkInboundMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let DingTalkInboundMessage {
        session,
        message,
        conversation_id,
        conversation_type,
        sender_user_id,
        sender_staff_id,
        session_webhook,
        session_webhook_expired_time,
        robot_code,
    } = inbound;

    let text = match &message.content {
        MessageContent::Text(text) => text.trim(),
        _ => "",
    };

    if text.is_empty() {
        info!("Skip non-text DingTalk message for session {}", session.id);
        return Ok(());
    }

    let Some(node) = state.hub.get_online_nodes().await.into_iter().next() else {
        return Err("No online node available".into());
    };

    let workspace_hint = node.workspace.path.clone();
    let command = plan_dingtalk_command(state, text, &workspace_hint).await?;

    let task_context = TaskContext::new(
        UserId::from_string(
            sender_user_id
                .clone()
                .unwrap_or_else(|| session.channel_user_id.clone()),
        ),
        uhorse_protocol::SessionId::from_string(session.id.0.clone()),
        "dingtalk",
    )
    .with_intent(text.to_string());

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

    {
        let mut routes = state.dingtalk_routes.write().await;
        routes.insert(
            task_id.clone(),
            DingTalkReplyRoute {
                conversation_id,
                conversation_type,
                sender_user_id,
                sender_staff_id,
                session_webhook,
                session_webhook_expired_time,
                robot_code,
            },
        );
    }

    info!(
        "Submitted DingTalk task {} for conversation {}",
        task_id,
        session.channel_user_id
    );

    Ok(())
}

async fn plan_dingtalk_command(
    state: &Arc<WebState>,
    text: &str,
    workspace_root: &str,
) -> Result<Command, Box<dyn std::error::Error + Send + Sync>> {
    let Some(llm_client) = state.llm_client.as_ref() else {
        return Err("LLM client is not configured".into());
    };

    let response = llm_client
        .chat_completion(build_dingtalk_plan_messages(text, workspace_root))
        .await?;

    parse_planned_command(&response, workspace_root)
}

fn build_dingtalk_plan_messages(text: &str, workspace_root: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(
            "你是 uHorse Hub 的任务规划器。你必须把用户的自然语言请求转换为单个 JSON 对象，且只能输出 JSON，不要输出 Markdown、解释或代码块。JSON 结构必须是 {\"command\": <uhorse_protocol::Command JSON> }。优先生成 file 命令；只有文件命令无法完成时才生成 shell 命令。禁止生成 code/database/api/browser/skill 命令。路径必须限制在 workspace 内，不允许绝对路径越界，不允许使用 ..。禁止危险 git：git reset --hard、git clean -fd、git checkout --、git restore --source、git push --force、git push -f。如果无法安全规划，返回一个会在本地校验失败的命令并附带原因到路径字段之外是不允许的，因此应返回最接近且可解析的安全命令。shell 命令只允许只读、安全的本地仓库检查或目录查看。".to_string(),
        ),
        ChatMessage::user(format!(
            "workspace_root: {}\nuser_request: {}\n请输出单个 JSON 对象。",
            workspace_root, text
        )),
    ]
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

pub async fn reply_task_result(
    state: Arc<WebState>,
    task_result: TaskResult,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    let reply_text = build_task_result_reply_text(&state, &task_result).await;
    send_dingtalk_reply(channel, &route, &reply_text).await?;

    info!("Replied DingTalk task result for {}", task_result.task_id);
    Ok(())
}

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
        CommandOutput::Json { content } => serde_json::to_string_pretty(content)
            .unwrap_or_else(|_| content.to_string()),
        CommandOutput::None => "执行成功，无输出。".to_string(),
        other => format!("执行成功，输出类型：{:?}", other),
    }
}

/// 获取统计信息
async fn get_stats(State(state): State<Arc<WebState>>) -> Json<ApiResponse<HubStats>> {
    match state.hub.get_stats().await {
        stats => Json(ApiResponse::success(stats)),
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

/// 列出任务
async fn list_tasks(State(_state): State<Arc<WebState>>) -> Json<ApiResponse<Vec<TaskInfo>>> {
    // 简化实现：返回空列表
    Json(ApiResponse::success(vec![]))
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
                command_type: "Shell".to_string(),
                priority: "Normal".to_string(),
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

/// 健康检查
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
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
    use tempfile::tempdir;
    use uhorse_protocol::{CommandResult, ExecutionError, ShellCommand};

    use crate::HubConfig;

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

    #[test]
    fn test_web_config_default() {
        let config = WebConfig::default();
        assert_eq!(config.port, 3000);
        assert!(config.enable_cors);
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
        let state = Arc::new(WebState::new(Arc::new(hub), None, Some(Arc::new(FailingLlmClient))));
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

        let command = plan_dingtalk_command(&state, "检查 Cargo.toml 是否存在", &workspace_root)
            .await
            .unwrap();

        match command {
            Command::File(FileCommand::Exists { path }) => {
                assert_eq!(path, format!("{}/Cargo.toml", workspace_root));
            }
            other => panic!("unexpected command: {:?}", other),
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
}
