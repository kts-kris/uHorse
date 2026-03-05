//! # Stream Handlers
//!
//! SSE 流式响应和实时通信端点。

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;

/// SSE 连接查询参数
#[derive(Debug, Deserialize)]
pub struct SseQuery {
    /// 最后事件 ID（用于重连）
    #[serde(rename = "lastEventId")]
    pub last_event_id: Option<String>,
    /// 订阅的房间（逗号分隔）
    pub rooms: Option<String>,
}

/// LLM 流式请求
#[derive(Debug, Deserialize)]
pub struct StreamChatRequest {
    /// 会话 ID
    pub session_id: String,
    /// 消息内容
    pub message: String,
    /// 模型
    pub model: Option<String>,
}

/// SSE 事件数据
#[derive(Debug, Serialize)]
pub struct SseEventData {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// SSE 事件端点
#[axum::debug_handler]
pub async fn sse_events(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<SseQuery>,
) -> impl IntoResponse {
    info!("SSE connection established, last_event_id={:?}", query.last_event_id);

    // 订阅 WebSocket 管理器的事件
    let mut event_rx = state.ws_manager.subscribe_events();

    // 创建 SSE 流
    let stream = async_stream::stream! {
        // 发送连接事件
        yield Ok::<Event, Infallible>(Event::default()
            .event("connected")
            .data(r#"{"status":"connected"}"#));

        loop {
            // 接收事件
            match event_rx.recv().await {
                Ok(ws_event) => {
                    // 转换为 SSE 事件
                    let (event_type, data) = match &ws_event {
                        crate::websocket::WsEvent::Message { session_id, role, content, timestamp } => {
                            ("message", serde_json::json!({
                                "session_id": session_id,
                                "role": role,
                                "content": content,
                                "timestamp": timestamp
                            }))
                        }
                        crate::websocket::WsEvent::StateChange { entity_type, entity_id, old_state, new_state } => {
                            ("state_change", serde_json::json!({
                                "entity_type": entity_type,
                                "entity_id": entity_id,
                                "old_state": old_state,
                                "new_state": new_state
                            }))
                        }
                        crate::websocket::WsEvent::TaskProgress { task_id, progress, message } => {
                            ("task_progress", serde_json::json!({
                                "task_id": task_id,
                                "progress": progress,
                                "message": message
                            }))
                        }
                        crate::websocket::WsEvent::AgentStatus { agent_id, status, message } => {
                            ("agent_status", serde_json::json!({
                                "agent_id": agent_id,
                                "status": status,
                                "message": message
                            }))
                        }
                        crate::websocket::WsEvent::ChannelStatus { channel_type, connected, message } => {
                            ("channel_status", serde_json::json!({
                                "channel_type": channel_type,
                                "connected": connected,
                                "message": message
                            }))
                        }
                        crate::websocket::WsEvent::Pong { timestamp } => {
                            ("pong", serde_json::json!({ "timestamp": timestamp }))
                        }
                        crate::websocket::WsEvent::Error { code, message } => {
                            ("error", serde_json::json!({
                                "code": code,
                                "message": message
                            }))
                        }
                    };

                    yield Ok::<Event, Infallible>(Event::default()
                        .event(event_type)
                        .data(data.to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!("SSE client lagged by {} messages", n);
                    yield Ok::<Event, Infallible>(Event::default()
                        .event("error")
                        .data(r#"{"code":"LAGGED","message":"Missed events"}"#));
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// LLM 流式聊天端点
#[axum::debug_handler]
pub async fn stream_chat(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<StreamChatRequest>,
) -> impl IntoResponse {
    info!("Stream chat request: session_id={}", req.session_id);

    // 创建模拟的 LLM 流式响应
    let words = vec![
        "你好", "！", "我", "是", " uHorse", " AI", " 助手", "。",
        "\n\n",
        "我", "可以", "帮助", "你", "处理", "各种", "任务", "。",
        "请", "告诉", "我", "你", "需要", "什么", "帮助", "。",
    ];

    let session_id = req.session_id.clone();
    let word_count = words.len();

    let stream = async_stream::stream! {
        for (i, word) in words.into_iter().enumerate() {
            // 模拟延迟
            tokio::time::sleep(Duration::from_millis(100)).await;

            let chunk = serde_json::json!({
                "session_id": session_id,
                "content": word,
                "done": i == word_count - 1
            });

            yield Ok::<Event, Infallible>(Event::default()
                .event("chunk")
                .data(chunk.to_string()));
        }

        // 发送完成事件
        yield Ok::<Event, Infallible>(Event::default()
            .event("done")
            .data(r#"{"status":"completed"}"#));
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// 获取活跃连接数
#[axum::debug_handler]
pub async fn get_active_connections(
    State(state): State<Arc<HttpState>>,
) -> impl IntoResponse {
    let count = state.ws_manager.active_connections().await;
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({
            "active_connections": count
        }))),
    )
}
