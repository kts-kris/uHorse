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
use uhorse_llm::OpenAIClient;
use uhorse_protocol::{Command, CommandOutput, FileCommand, TaskContext, TaskId, UserId};

use crate::{Hub, HubStats};
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

/// 允许的 DingTalk 管理动作
#[derive(Debug, Clone)]
enum AllowedDingTalkCommand {
    List { path: String, recursive: bool },
    Search { path: String, pattern: String },
    Read { path: String, limit: Option<usize> },
    Info { path: String },
    Exists { path: String },
}

/// Web 服务器状态
#[derive(Clone)]
pub struct WebState {
    /// Hub 引用
    pub hub: Arc<Hub>,
    /// DingTalk 通道
    pub dingtalk_channel: Option<Arc<DingTalkChannel>>,
    /// LLM 客户端
    pub llm_client: Option<Arc<OpenAIClient>>,
    /// 任务回传路由
    pub dingtalk_routes: Arc<RwLock<HashMap<TaskId, DingTalkReplyRoute>>>,
}

impl WebState {
    /// 创建新的 Web 状态
    pub fn new(
        hub: Arc<Hub>,
        dingtalk_channel: Option<Arc<DingTalkChannel>>,
        llm_client: Option<Arc<OpenAIClient>>,
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

    let allowed_command = parse_allowed_dingtalk_command(text)?;
    let workspace_hint = node.workspace.path.clone();
    let command = build_dingtalk_command(&allowed_command, &workspace_hint);

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

fn parse_allowed_dingtalk_command(
    text: &str,
) -> Result<AllowedDingTalkCommand, Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.is_empty() {
        return Err("空命令".into());
    }

    match parts[0] {
        "list" | "ls" => {
            if parts.len() < 2 {
                return Err("用法：list <path> [--recursive]".into());
            }
            Ok(AllowedDingTalkCommand::List {
                path: normalize_relative_path(parts[1])?,
                recursive: parts.iter().skip(2).any(|arg| *arg == "--recursive" || *arg == "-r"),
            })
        }
        "search" => {
            if parts.len() < 3 {
                return Err("用法：search <path> <glob-pattern>".into());
            }
            Ok(AllowedDingTalkCommand::Search {
                path: normalize_relative_path(parts[1])?,
                pattern: parts[2].to_string(),
            })
        }
        "read" | "cat" => {
            if parts.len() < 2 {
                return Err("用法：read <path> [limit]".into());
            }
            let limit = parts
                .get(2)
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| "limit 必须是数字")?;
            Ok(AllowedDingTalkCommand::Read {
                path: normalize_relative_path(parts[1])?,
                limit,
            })
        }
        "info" => {
            if parts.len() < 2 {
                return Err("用法：info <path>".into());
            }
            Ok(AllowedDingTalkCommand::Info {
                path: normalize_relative_path(parts[1])?,
            })
        }
        "exists" => {
            if parts.len() < 2 {
                return Err("用法：exists <path>".into());
            }
            Ok(AllowedDingTalkCommand::Exists {
                path: normalize_relative_path(parts[1])?,
            })
        }
        _ => Err(
            "仅支持白名单管理命令：list/ls、search、read/cat、info、exists。".into(),
        ),
    }
}

fn build_dingtalk_command(command: &AllowedDingTalkCommand, workspace_root: &str) -> Command {
    match command {
        AllowedDingTalkCommand::List { path, recursive } => Command::File(FileCommand::List {
            path: workspace_path(workspace_root, path),
            recursive: *recursive,
            pattern: None,
        }),
        AllowedDingTalkCommand::Search { path, pattern } => Command::File(FileCommand::Search {
            pattern: pattern.clone(),
            path: workspace_path(workspace_root, path),
            recursive: true,
            content_pattern: None,
        }),
        AllowedDingTalkCommand::Read { path, limit } => Command::File(FileCommand::Read {
            path: workspace_path(workspace_root, path),
            limit: Some(limit.unwrap_or(4000)),
            offset: None,
        }),
        AllowedDingTalkCommand::Info { path } => Command::File(FileCommand::Info {
            path: workspace_path(workspace_root, path),
        }),
        AllowedDingTalkCommand::Exists { path } => Command::File(FileCommand::Exists {
            path: workspace_path(workspace_root, path),
        }),
    }
}

fn workspace_path(workspace_root: &str, relative_path: &str) -> String {
    let mut buf = PathBuf::from(workspace_root);
    if relative_path != "." {
        buf.push(relative_path);
    }
    buf.to_string_lossy().to_string()
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

fn extract_sender_user_id(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("senderId")?
        .as_str()
        .map(|value| value.to_string())
}

fn extract_conversation_type(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("conversationType")?
        .as_str()
        .map(|value| value.to_string())
}

pub async fn reply_task_result(
    state: Arc<WebState>,
    task_result: crate::task_scheduler::TaskResult,
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

    let reply_text = format_task_result_message(&task_result.result);
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

    #[test]
    fn test_web_config_default() {
        let config = WebConfig::default();
        assert_eq!(config.port, 3000);
        assert!(config.enable_cors);
    }
}
