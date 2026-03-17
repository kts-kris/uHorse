//! Debug Panel Module
//!
//! Provides a web-based debugging interface for monitoring conversations,
//! tool calls, and performance metrics.

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

mod websocket;
mod metrics;

pub use websocket::*;
pub use metrics::*;

/// Maximum events to keep in memory
const MAX_EVENTS: usize = 1000;

/// Debug panel state
#[derive(Debug, Clone)]
pub struct DebugPanelState {
    /// Event history
    pub events: Arc<RwLock<VecDeque<DebugEvent>>>,
    /// Event broadcaster for real-time updates
    pub event_tx: broadcast::Sender<DebugEvent>,
    /// Metrics collector
    pub metrics: Arc<RwLock<DebugMetrics>>,
}

impl Default for DebugPanelState {
    fn default() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_EVENTS))),
            event_tx,
            metrics: Arc::new(RwLock::new(DebugMetrics::default())),
        }
    }
}

/// Debug event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebugEvent {
    /// Conversation message
    ConversationMessage {
        session_id: String,
        channel: String,
        role: String,
        content: String,
        timestamp: i64,
    },
    /// Tool call
    ToolCall {
        session_id: String,
        tool_name: String,
        parameters: serde_json::Value,
        result: Option<serde_json::Value>,
        duration_ms: u64,
        success: bool,
        timestamp: i64,
    },
    /// LLM request
    LlmRequest {
        session_id: String,
        provider: String,
        model: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        duration_ms: u64,
        timestamp: i64,
    },
    /// Error
    Error {
        session_id: Option<String>,
        error_type: String,
        message: String,
        stack_trace: Option<String>,
        timestamp: i64,
    },
    /// Performance metric
    PerformanceMetric {
        metric_name: String,
        value: f64,
        unit: String,
        tags: std::collections::HashMap<String, String>,
        timestamp: i64,
    },
    /// Session event
    SessionEvent {
        session_id: String,
        event: String,
        details: Option<serde_json::Value>,
        timestamp: i64,
    },
}

/// Aggregated debug metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebugMetrics {
    /// Total events
    pub total_events: u64,
    /// Active sessions
    pub active_sessions: u64,
    /// Tool calls in last hour
    pub tool_calls_hour: u64,
    /// LLM requests in last hour
    pub llm_requests_hour: u64,
    /// Average response time (ms)
    pub avg_response_time_ms: f64,
    /// Error rate
    pub error_rate: f64,
    /// Token usage
    pub token_usage: TokenUsage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Query parameters for event filtering
#[derive(Debug, Deserialize)]
pub struct EventQuery {
    /// Filter by session ID
    pub session_id: Option<String>,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Maximum events to return
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

/// Debug panel router
pub fn debug_panel_router(state: Arc<DebugPanelState>) -> Router {
    Router::new()
        // UI
        .route("/debug", get(debug_ui))
        .route("/debug/sessions", get(debug_sessions_ui))
        .route("/debug/tools", get(debug_tools_ui))
        .route("/debug/performance", get(debug_performance_ui))
        // API
        .route("/api/debug/events", get(get_events))
        .route("/api/debug/events", post(record_event))
        .route("/api/debug/events/:session_id", get(get_session_events))
        .route("/api/debug/metrics", get(get_metrics))
        .route("/api/debug/sessions", get(get_sessions))
        .route("/api/debug/sessions/:session_id", get(get_session_detail))
        .route("/api/debug/tools", get(get_tool_stats))
        .route("/api/debug/performance", get(get_performance_stats))
        // WebSocket
        .route("/api/debug/ws", get(websocket_handler))
        .with_state(state)
}

// UI Handlers

async fn debug_ui() -> impl IntoResponse {
    Html(include_str!("ui/index.html"))
}

async fn debug_sessions_ui() -> impl IntoResponse {
    Html(include_str!("ui/sessions.html"))
}

async fn debug_tools_ui() -> impl IntoResponse {
    Html(include_str!("ui/tools.html"))
}

async fn debug_performance_ui() -> impl IntoResponse {
    Html(include_str!("ui/performance.html"))
}

// API Handlers

/// Get debug events
async fn get_events(
    State(state): State<Arc<DebugPanelState>>,
    Query(query): Query<EventQuery>,
) -> impl IntoResponse {
    let events = state.events.read().await;

    let filtered: Vec<_> = events
        .iter()
        .filter(|event| {
            if let Some(ref session_id) = query.session_id {
                if !event_matches_session(event, session_id) {
                    return false;
                }
            }
            if let Some(ref event_type) = query.event_type {
                if !event_matches_type(event, event_type) {
                    return false;
                }
            }
            true
        })
        .skip(query.offset)
        .take(query.limit)
        .cloned()
        .collect();

    Json(serde_json::json!({
        "success": true,
        "events": filtered,
        "total": events.len()
    }))
}

/// Record a debug event
async fn record_event(
    State(state): State<Arc<DebugPanelState>>,
    Json(event): Json<DebugEvent>,
) -> impl IntoResponse {
    // Add to history
    {
        let mut events = state.events.write().await;
        if events.len() >= MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event.clone());
    }

    // Broadcast to WebSocket clients
    let _ = state.event_tx.send(event);

    // Update metrics
    {
        let mut metrics = state.metrics.write().await;
        update_metrics(&mut metrics, &event);
    }

    Json(serde_json::json!({
        "success": true
    }))
}

/// Get events for a specific session
async fn get_session_events(
    State(state): State<Arc<DebugPanelState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let events = state.events.read().await;

    let session_events: Vec<_> = events
        .iter()
        .filter(|event| event_matches_session(event, &session_id))
        .cloned()
        .collect();

    Json(serde_json::json!({
        "success": true,
        "events": session_events
    }))
}

/// Get current metrics
async fn get_metrics(
    State(state): State<Arc<DebugPanelState>>,
) -> impl IntoResponse {
    let metrics = state.metrics.read().await.clone();
    Json(serde_json::json!({
        "success": true,
        "metrics": metrics
    }))
}

/// Get active sessions
async fn get_sessions(
    State(state): State<Arc<DebugPanelState>>,
) -> impl IntoResponse {
    let events = state.events.read().await;

    let mut sessions = std::collections::HashSet::new();
    for event in events.iter() {
        if let Some(session_id) = get_session_id(event) {
            sessions.insert(session_id);
        }
    }

    Json(serde_json::json!({
        "success": true,
        "sessions": sessions
    }))
}

/// Get session detail
async fn get_session_detail(
    State(state): State<Arc<DebugPanelState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let events = state.events.read().await;

    let session_events: Vec<_> = events
        .iter()
        .filter(|event| event_matches_session(event, &session_id))
        .cloned()
        .collect();

    // Calculate session stats
    let mut stats = SessionStats::default();
    for event in &session_events {
        match event {
            DebugEvent::ToolCall { duration_ms, .. } => {
                stats.tool_calls += 1;
                stats.total_tool_time_ms += duration_ms;
            }
            DebugEvent::LlmRequest { prompt_tokens, completion_tokens, .. } => {
                stats.llm_requests += 1;
                stats.prompt_tokens += prompt_tokens;
                stats.completion_tokens += completion_tokens;
            }
            DebugEvent::Error { .. } => {
                stats.errors += 1;
            }
            _ => {}
        }
    }

    Json(serde_json::json!({
        "success": true,
        "session_id": session_id,
        "events": session_events,
        "stats": stats
    }))
}

/// Get tool statistics
async fn get_tool_stats(
    State(state): State<Arc<DebugPanelState>>,
) -> impl IntoResponse {
    let events = state.events.read().await;

    let mut tool_stats: std::collections::HashMap<String, ToolStats> =
        std::collections::HashMap::new();

    for event in events.iter() {
        if let DebugEvent::ToolCall { tool_name, duration_ms, success, .. } = event {
            let stats = tool_stats.entry(tool_name.clone()).or_default();
            stats.calls += 1;
            stats.total_time_ms += duration_ms;
            if !success {
                stats.failures += 1;
            }
        }
    }

    Json(serde_json::json!({
        "success": true,
        "tools": tool_stats
    }))
}

/// Get performance statistics
async fn get_performance_stats(
    State(state): State<Arc<DebugPanelState>>,
) -> impl IntoResponse {
    let metrics = state.metrics.read().await.clone();
    let events = state.events.read().await;

    // Calculate performance data points
    let mut response_times: Vec<u64> = Vec::new();
    for event in events.iter() {
        if let DebugEvent::LlmRequest { duration_ms, .. } = event {
            response_times.push(*duration_ms);
        }
    }

    Json(serde_json::json!({
        "success": true,
        "metrics": metrics,
        "response_times": response_times
    }))
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    State(state): State<Arc<DebugPanelState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

// Helper functions

fn event_matches_session(event: &DebugEvent, session_id: &str) -> bool {
    match event {
        DebugEvent::ConversationMessage { session_id: sid, .. } => sid == session_id,
        DebugEvent::ToolCall { session_id: sid, .. } => sid == session_id,
        DebugEvent::LlmRequest { session_id: sid, .. } => sid == session_id,
        DebugEvent::Error { session_id: Some(sid), .. } => sid == session_id,
        DebugEvent::SessionEvent { session_id: sid, .. } => sid == session_id,
        _ => false,
    }
}

fn event_matches_type(event: &DebugEvent, event_type: &str) -> bool {
    let actual_type = match event {
        DebugEvent::ConversationMessage { .. } => "conversation_message",
        DebugEvent::ToolCall { .. } => "tool_call",
        DebugEvent::LlmRequest { .. } => "llm_request",
        DebugEvent::Error { .. } => "error",
        DebugEvent::PerformanceMetric { .. } => "performance_metric",
        DebugEvent::SessionEvent { .. } => "session_event",
    };
    actual_type == event_type
}

fn get_session_id(event: &DebugEvent) -> Option<String> {
    match event {
        DebugEvent::ConversationMessage { session_id, .. } => Some(session_id.clone()),
        DebugEvent::ToolCall { session_id, .. } => Some(session_id.clone()),
        DebugEvent::LlmRequest { session_id, .. } => Some(session_id.clone()),
        DebugEvent::Error { session_id: Some(sid), .. } => Some(sid.clone()),
        DebugEvent::SessionEvent { session_id, .. } => Some(session_id.clone()),
        _ => None,
    }
}

fn update_metrics(metrics: &mut DebugMetrics, event: &DebugEvent) {
    metrics.total_events += 1;

    match event {
        DebugEvent::LlmRequest { prompt_tokens, completion_tokens, .. } => {
            metrics.llm_requests_hour += 1;
            metrics.token_usage.prompt_tokens += *prompt_tokens as u64;
            metrics.token_usage.completion_tokens += *completion_tokens as u64;
            metrics.token_usage.total_tokens += (*prompt_tokens + *completion_tokens) as u64;
        }
        DebugEvent::ToolCall { .. } => {
            metrics.tool_calls_hour += 1;
        }
        DebugEvent::Error { .. } => {
            // Update error rate
            if metrics.total_events > 0 {
                metrics.error_rate = (metrics.error_rate * (metrics.total_events - 1) as f64 + 1.0)
                    / metrics.total_events as f64;
            }
        }
        _ => {}
    }
}

#[derive(Debug, Default, Serialize)]
struct SessionStats {
    tool_calls: u64,
    total_tool_time_ms: u64,
    llm_requests: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    errors: u64,
}

#[derive(Debug, Default, Serialize)]
struct ToolStats {
    calls: u64,
    total_time_ms: u64,
    failures: u64,
}
