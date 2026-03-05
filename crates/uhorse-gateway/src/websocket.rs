//! # WebSocket 处理器
//!
//! 实现 WebSocket 协议处理逻辑：
//! - 连接管理（心跳、重连、会话绑定）
//! - 事件推送（消息、状态变更、任务进度）
//! - 房间机制（按 Agent/Session 分组）

use axum::{
    extract::{
        ws::{Message as AxumMessage, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::http::HttpState;

/// WebSocket 连接配置
#[derive(Debug, Deserialize)]
pub struct WsConnectQuery {
    /// 客户端 ID
    pub client_id: Option<String>,
    /// 会话 ID（绑定到特定会话）
    pub session_id: Option<String>,
    /// Agent ID（绑定到特定 Agent）
    pub agent_id: Option<String>,
}

/// WebSocket 连接信息
#[derive(Debug, Clone)]
pub struct Connection {
    /// 连接 ID
    pub id: String,
    /// 客户端 ID
    pub client_id: String,
    /// 绑定的会话 ID
    pub session_id: Option<String>,
    /// 绑定的 Agent ID
    pub agent_id: Option<String>,
    /// 连接时间
    pub connected_at: u64,
}

/// WebSocket 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    /// 消息事件
    #[serde(rename = "message")]
    Message {
        session_id: String,
        role: String,
        content: String,
        timestamp: u64,
    },
    /// 状态变更事件
    #[serde(rename = "state_change")]
    StateChange {
        entity_type: String,
        entity_id: String,
        old_state: String,
        new_state: String,
    },
    /// 任务进度事件
    #[serde(rename = "task_progress")]
    TaskProgress {
        task_id: String,
        progress: f32,
        message: String,
    },
    /// Agent 状态事件
    #[serde(rename = "agent_status")]
    AgentStatus {
        agent_id: String,
        status: String,
        message: Option<String>,
    },
    /// 通道状态事件
    #[serde(rename = "channel_status")]
    ChannelStatus {
        channel_type: String,
        connected: bool,
        message: Option<String>,
    },
    /// 心跳响应
    #[serde(rename = "pong")]
    Pong { timestamp: u64 },
    /// 错误事件
    #[serde(rename = "error")]
    Error { code: String, message: String },
}

/// WebSocket 命令
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum WsCommand {
    /// 心跳
    #[serde(rename = "ping")]
    Ping { timestamp: u64 },
    /// 订阅房间
    #[serde(rename = "subscribe")]
    Subscribe { room: String },
    /// 取消订阅
    #[serde(rename = "unsubscribe")]
    Unsubscribe { room: String },
    /// 发送消息
    #[serde(rename = "send")]
    Send { session_id: String, content: String },
}

/// 房间类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Room {
    /// 全局事件
    Global,
    /// Agent 房间
    Agent(String),
    /// Session 房间
    Session(String),
}

impl Room {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "global" {
            Some(Room::Global)
        } else {
            s.strip_prefix("agent:")
                .map(|id| Room::Agent(id.to_string()))
                .or_else(|| {
                    s.strip_prefix("session:")
                        .map(|id| Room::Session(id.to_string()))
                })
        }
    }
}

impl std::fmt::Display for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Room::Global => write!(f, "global"),
            Room::Agent(id) => write!(f, "agent:{}", id),
            Room::Session(id) => write!(f, "session:{}", id),
        }
    }
}

/// 连接管理器
#[derive(Debug)]
pub struct ConnectionManager {
    /// 活跃连接
    connections: Arc<RwLock<HashMap<String, Connection>>>,
    /// 房间订阅
    room_subscriptions: Arc<RwLock<HashMap<Room, Vec<String>>>>,
    /// 事件广播通道
    event_tx: broadcast::Sender<WsEvent>,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            room_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// 获取事件订阅器
    pub fn subscribe_events(&self) -> broadcast::Receiver<WsEvent> {
        self.event_tx.subscribe()
    }

    /// 广播事件
    pub async fn broadcast(&self, event: WsEvent) {
        if let Err(e) = self.event_tx.send(event) {
            warn!("Failed to broadcast event: {}", e);
        }
    }

    /// 广播事件到特定房间
    pub async fn broadcast_to_room(&self, room: &Room, event: WsEvent) {
        let subscriptions = self.room_subscriptions.read().await;
        if let Some(connection_ids) = subscriptions.get(room) {
            debug!(
                "Broadcasting to room {:?}: {} connections",
                room,
                connection_ids.len()
            );
            // 事件会通过 broadcast channel 传给所有订阅者
        }
        if let Err(e) = self.event_tx.send(event) {
            warn!("Failed to broadcast to room: {}", e);
        }
    }

    /// 添加连接
    pub async fn add_connection(&self, connection: Connection) {
        let id = connection.id.clone();
        self.connections.write().await.insert(id, connection);
        info!("WebSocket connection added");
    }

    /// 移除连接
    pub async fn remove_connection(&self, id: &str) {
        if let Some(conn) = self.connections.write().await.remove(id) {
            // 从所有房间移除订阅
            let mut subs = self.room_subscriptions.write().await;
            for conns in subs.values_mut() {
                conns.retain(|cid| cid != id);
            }
            info!("WebSocket connection removed: client={}", conn.client_id);
        }
    }

    /// 订阅房间
    pub async fn subscribe_room(&self, connection_id: &str, room: Room) {
        debug!(
            "Connection {} subscribing to room {:?}",
            connection_id, &room
        );
        self.room_subscriptions
            .write()
            .await
            .entry(room)
            .or_insert_with(Vec::new)
            .push(connection_id.to_string());
    }

    /// 取消订阅房间
    pub async fn unsubscribe_room(&self, connection_id: &str, room: &Room) {
        if let Some(conns) = self.room_subscriptions.write().await.get_mut(room) {
            conns.retain(|id| id != connection_id);
        }
        debug!(
            "Connection {} unsubscribed from room {:?}",
            connection_id, room
        );
    }

    /// 获取活跃连接数
    pub async fn active_connections(&self) -> usize {
        self.connections.read().await.len()
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 处理 WebSocket 升级
pub async fn handle_upgrade(
    State(state): State<Arc<HttpState>>,
    Query(query): Query<WsConnectQuery>,
    ws: WebSocketUpgrade,
) -> axum::response::Response<axum::body::Body> {
    debug!("WebSocket upgrade request: {:?}", query);

    ws.on_upgrade(move |socket| handle_socket(socket, state.ws_manager.clone(), query))
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, manager: Arc<ConnectionManager>, query: WsConnectQuery) {
    let connection_id = Uuid::new_v4().to_string();
    let client_id = query
        .client_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let connection = Connection {
        id: connection_id.clone(),
        client_id: client_id.clone(),
        session_id: query.session_id.clone(),
        agent_id: query.agent_id.clone(),
        connected_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // 添加连接
    manager.add_connection(connection.clone()).await;

    // 自动订阅相关房间
    if let Some(ref agent_id) = query.agent_id {
        manager
            .subscribe_room(&connection_id, Room::Agent(agent_id.clone()))
            .await;
    }
    if let Some(ref session_id) = query.session_id {
        manager
            .subscribe_room(&connection_id, Room::Session(session_id.clone()))
            .await;
    }

    info!(
        "WebSocket connected: id={}, client={}, agent={:?}, session={:?}",
        connection_id, client_id, query.agent_id, query.session_id
    );

    // 分割 socket 为发送和接收
    let (mut sender, mut receiver) = socket.split();

    // 订阅事件广播
    let mut event_rx = manager.subscribe_events();
    let conn_id = connection_id.clone();
    let mgr = manager.clone();

    // 发送任务：接收广播事件并发送给客户端
    let send_task = async move {
        while let Ok(event) = event_rx.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(AxumMessage::Text(json)).await.is_err() {
                break;
            }
        }
    };

    // 接收任务：处理客户端消息
    let recv_task = async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(AxumMessage::Text(text)) => {
                    if let Err(e) = handle_client_message(&mgr, &conn_id, &text).await {
                        warn!("Failed to handle client message: {}", e);
                    }
                }
                Ok(AxumMessage::Ping(data)) => {
                    debug!("Received ping: {:?}", data);
                }
                Ok(AxumMessage::Pong(_)) => {
                    debug!("Received pong");
                }
                Ok(AxumMessage::Close(_)) => {
                    info!("Client closed connection: {}", conn_id);
                    break;
                }
                Err(e) => {
                    warn!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    // 并行运行发送和接收任务
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // 清理连接
    manager.remove_connection(&connection_id).await;
    info!("WebSocket disconnected: id={}", connection_id);
}

/// 处理客户端消息
async fn handle_client_message(
    manager: &ConnectionManager,
    connection_id: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let command: WsCommand = serde_json::from_str(text)?;

    match command {
        WsCommand::Ping { timestamp } => {
            let pong = WsEvent::Pong { timestamp };
            manager.broadcast(pong).await;
        }
        WsCommand::Subscribe { room } => {
            if let Some(room) = Room::parse(&room) {
                let room_str = room.to_string();
                manager.subscribe_room(connection_id, room).await;
                // 发送确认
                let confirm = WsEvent::StateChange {
                    entity_type: "subscription".to_string(),
                    entity_id: room_str,
                    old_state: "unsubscribed".to_string(),
                    new_state: "subscribed".to_string(),
                };
                manager.broadcast(confirm).await;
            }
        }
        WsCommand::Unsubscribe { room } => {
            if let Some(room) = Room::parse(&room) {
                manager.unsubscribe_room(connection_id, &room).await;
            }
        }
        WsCommand::Send {
            session_id,
            content,
        } => {
            // 广播消息事件
            let event = WsEvent::Message {
                session_id,
                role: "user".to_string(),
                content,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            manager.broadcast(event).await;
        }
    }

    Ok(())
}

// ============================================================================
// SSE 流式响应
// ============================================================================

/// SSE 事件
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
    pub id: Option<String>,
}

impl SseEvent {
    pub fn new(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            data: data.into(),
            id: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn format_sse(&self) -> String {
        let mut result = format!("event: {}\n", self.event);
        for line in self.data.lines() {
            result.push_str(&format!("data: {}\n", line));
        }
        if let Some(ref id) = self.id {
            result.push_str(&format!("id: {}\n", id));
        }
        result.push('\n');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_parsing() {
        assert_eq!(Room::parse("global"), Some(Room::Global));
        assert_eq!(
            Room::parse("agent:123"),
            Some(Room::Agent("123".to_string()))
        );
        assert_eq!(
            Room::parse("session:456"),
            Some(Room::Session("456".to_string()))
        );
        assert_eq!(Room::parse("invalid"), None);
    }

    #[test]
    fn test_room_to_string() {
        assert_eq!(Room::Global.to_string(), "global");
        assert_eq!(Room::Agent("123".to_string()).to_string(), "agent:123");
        assert_eq!(Room::Session("456".to_string()).to_string(), "session:456");
    }

    #[test]
    fn test_sse_event() {
        let event = SseEvent::new("message", "Hello World");
        let s = event.format_sse();
        assert!(s.contains("event: message"));
        assert!(s.contains("data: Hello World"));
    }

    #[test]
    fn test_sse_event_multiline() {
        let event = SseEvent::new("message", "Line 1\nLine 2");
        let s = event.format_sse();
        assert!(s.contains("data: Line 1"));
        assert!(s.contains("data: Line 2"));
    }

    #[test]
    fn test_ws_command_parsing() {
        let json = r#"{"type":"ping","timestamp":12345}"#;
        let cmd: WsCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, WsCommand::Ping { timestamp: 12345 }));
    }

    #[test]
    fn test_ws_event_serialization() {
        let event = WsEvent::Message {
            session_id: "s123".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: 12345,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"message""#));
    }
}
