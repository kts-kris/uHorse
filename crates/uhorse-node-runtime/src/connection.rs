//! Hub 连接管理
//!
//! 管理与云端中枢的 WebSocket 连接

use crate::error::{NodeError, NodeResult};
use crate::status::HeartbeatSnapshot;
use chrono::{DateTime, Utc};
use futures::{Sink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};
use uhorse_protocol::{HubToNode, MessageCodec, NodeCapabilities, NodeId, NodeToHub};

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Hub WebSocket URL
    pub hub_url: String,

    /// 重连间隔（秒）
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval_secs: u64,

    /// 心跳间隔（秒）
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// 连接超时（秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,

    /// 最大重连次数
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,

    /// 认证令牌
    pub auth_token: Option<String>,
}

fn default_reconnect_interval() -> u64 {
    5
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_max_reconnect_attempts() -> u32 {
    10
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            hub_url: "ws://localhost:8765/ws".to_string(),
            reconnect_interval_secs: default_reconnect_interval(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            connect_timeout_secs: default_connect_timeout(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            auth_token: None,
        }
    }
}

/// 连接状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// 已断开
    Disconnected,

    /// 连接中
    Connecting,

    /// 已连接
    Connected {
        /// 连接时间
        connected_at: DateTime<Utc>,
    },

    /// 认证中
    Authenticating,

    /// 已认证
    Authenticated {
        /// 认证时间
        authenticated_at: DateTime<Utc>,
    },

    /// 重连中
    Reconnecting {
        /// 尝试次数
        attempt: u32,
    },

    /// 失败
    Failed {
        /// 错误信息
        error: String,
    },
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected { .. } => write!(f, "connected"),
            Self::Authenticating => write!(f, "authenticating"),
            Self::Authenticated { .. } => write!(f, "authenticated"),
            Self::Reconnecting { attempt } => write!(f, "reconnecting(attempt={})", attempt),
            Self::Failed { error } => write!(f, "failed({})", error),
        }
    }
}

/// Hub 连接
#[derive(Debug)]
pub struct HubConnection {
    /// 配置
    config: ConnectionConfig,

    /// 节点 ID
    node_id: NodeId,

    /// 节点名称
    node_name: String,

    /// 工作空间路径
    workspace_path: String,

    /// 节点能力
    capabilities: NodeCapabilities,

    /// 连接状态
    state: Arc<RwLock<ConnectionState>>,

    /// 是否运行中
    running: Arc<AtomicBool>,

    /// 最新节点状态
    heartbeat_snapshot: Arc<RwLock<Option<HeartbeatSnapshot>>>,

    /// 接收通道
    receiver: Option<mpsc::Receiver<HubToNode>>,
}

impl HubConnection {
    /// 创建新的 Hub 连接
    pub fn new(
        node_id: NodeId,
        config: ConnectionConfig,
        node_name: String,
        workspace_path: String,
        capabilities: NodeCapabilities,
        heartbeat_snapshot: Arc<RwLock<Option<HeartbeatSnapshot>>>,
    ) -> Self {
        Self {
            config,
            node_id,
            node_name,
            workspace_path,
            capabilities,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            running: Arc::new(AtomicBool::new(false)),
            heartbeat_snapshot,
            receiver: None,
        }
    }

    /// 启动连接
    pub async fn start(
        &mut self,
    ) -> NodeResult<(mpsc::Receiver<HubToNode>, mpsc::Sender<NodeToHub>)> {
        if self.running.load(Ordering::SeqCst) {
            return Err(NodeError::Connection(
                "Connection already running".to_string(),
            ));
        }

        let (inbound_tx, inbound_rx) = mpsc::channel(100);
        let (outbound_tx, outbound_rx) = mpsc::channel(100);
        self.receiver = Some(inbound_rx);

        self.running.store(true, Ordering::SeqCst);
        *self.state.write().await = ConnectionState::Connecting;

        // 启动连接循环
        let state = self.state.clone();
        let running = self.running.clone();
        let config = self.config.clone();
        let node_id = self.node_id.clone();
        let node_name = self.node_name.clone();
        let workspace_path = self.workspace_path.clone();
        let capabilities = self.capabilities.clone();
        let heartbeat_snapshot = self.heartbeat_snapshot.clone();

        tokio::spawn(async move {
            Self::connection_loop(
                state,
                running,
                config,
                node_id,
                node_name,
                workspace_path,
                capabilities,
                heartbeat_snapshot,
                inbound_tx,
                outbound_rx,
            )
            .await;
        });

        Ok((self.receiver.take().unwrap(), outbound_tx))
    }

    /// 连接循环
    #[allow(clippy::too_many_arguments)]
    async fn connection_loop(
        state: Arc<RwLock<ConnectionState>>,
        running: Arc<AtomicBool>,
        config: ConnectionConfig,
        node_id: NodeId,
        node_name: String,
        workspace_path: String,
        capabilities: NodeCapabilities,
        heartbeat_snapshot: Arc<RwLock<Option<HeartbeatSnapshot>>>,
        inbound_tx: mpsc::Sender<HubToNode>,
        outbound_rx: mpsc::Receiver<NodeToHub>,
    ) {
        let mut reconnect_attempts = 0;

        let outbound_rx = Arc::new(tokio::sync::Mutex::new(outbound_rx));

        while running.load(Ordering::SeqCst) {
            // 尝试连接
            match Self::connect_and_run(
                &state,
                &config,
                &node_id,
                &node_name,
                &workspace_path,
                &capabilities,
                &heartbeat_snapshot,
                &inbound_tx,
                &outbound_rx,
                &running,
            )
            .await
            {
                Ok(_) => {
                    // 连接正常关闭
                    reconnect_attempts = 0;
                    info!("Connection closed normally");
                }
                Err(e) => {
                    error!("Connection error: {}", e);

                    reconnect_attempts += 1;
                    if reconnect_attempts > config.max_reconnect_attempts {
                        *state.write().await = ConnectionState::Failed {
                            error: e.to_string(),
                        };
                        break;
                    }

                    *state.write().await = ConnectionState::Reconnecting {
                        attempt: reconnect_attempts,
                    };

                    // 等待重连
                    sleep(Duration::from_secs(config.reconnect_interval_secs)).await;
                }
            }
        }

        *state.write().await = ConnectionState::Disconnected;
        info!("Connection loop stopped");
    }

    /// 连接并运行
    #[allow(clippy::too_many_arguments)]
    async fn connect_and_run(
        state: &Arc<RwLock<ConnectionState>>,
        config: &ConnectionConfig,
        node_id: &NodeId,
        node_name: &str,
        workspace_path: &str,
        capabilities: &NodeCapabilities,
        heartbeat_snapshot: &Arc<RwLock<Option<HeartbeatSnapshot>>>,
        inbound_tx: &mpsc::Sender<HubToNode>,
        outbound_rx: &Arc<tokio::sync::Mutex<mpsc::Receiver<NodeToHub>>>,
        running: &Arc<AtomicBool>,
    ) -> NodeResult<()> {
        info!("Connecting to Hub: {}", config.hub_url);

        // 建立 WebSocket 连接
        let (ws_stream, _) = tokio::time::timeout(
            Duration::from_secs(config.connect_timeout_secs),
            connect_async(&config.hub_url),
        )
        .await
        .map_err(|_| NodeError::Timeout("Connection timeout".to_string()))?
        .map_err(|e| NodeError::Connection(format!("Failed to connect: {}", e)))?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // 更新状态
        *state.write().await = ConnectionState::Connected {
            connected_at: Utc::now(),
        };

        info!("Connected to Hub successfully");

        // 发送注册消息
        let workspace_name = std::path::Path::new(workspace_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();

        let register_msg = NodeToHub::Register {
            message_id: uhorse_protocol::MessageId::new(),
            node_id: node_id.clone(),
            name: node_name.to_string(),
            capabilities: capabilities.clone(),
            workspace: uhorse_protocol::WorkspaceInfo {
                workspace_id: None,
                name: workspace_name,
                path: workspace_path.to_string(),
                read_only: false,
                allowed_patterns: vec!["**/*".to_string()],
                denied_patterns: vec![],
            },
            auth_token: config.auth_token.clone().unwrap_or_default(),
            timestamp: Utc::now(),
        };

        Self::send_node_message(&mut ws_sender, &register_msg)
            .await
            .map_err(|e| {
                NodeError::Connection(format!("Failed to send register message: {}", e))
            })?;

        // 更新状态
        *state.write().await = ConnectionState::Authenticated {
            authenticated_at: Utc::now(),
        };

        // 接收消息循环
        while running.load(Ordering::SeqCst) {
            tokio::select! {
                // 接收消息
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(WsMessage::Binary(data))) => {
                            if let Ok(hub_msg) = MessageCodec::decode_hub_to_node(&data) {
                                debug!("Received message from Hub: {:?}", hub_msg.message_type());

                                if inbound_tx.send(hub_msg).await.is_err() {
                                    warn!("Failed to forward message to inbound channel");
                                    break;
                                }
                            } else {
                                warn!("Failed to decode message from Hub");
                            }
                        }
                        Some(Ok(WsMessage::Ping(data))) => {
                            if ws_sender.send(WsMessage::Pong(data)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            info!("Hub closed connection");
                            break;
                        }
                        Some(Ok(WsMessage::Pong(_))) => {
                            // Ignore pong
                        }
                        Some(Ok(_)) => {
                            // Ignore other message types
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                    }
                }

                // 发送业务出站消息
                outbound = Self::recv_outbound_message(outbound_rx) => {
                    match outbound {
                        Some(message) => {
                            if let Err(e) = Self::send_node_message(&mut ws_sender, &message).await {
                                warn!("Failed to send outbound message {}: {}", message.message_type(), e);
                                break;
                            }
                        }
                        None => {
                            debug!("Outbound channel closed");
                            break;
                        }
                    }
                }

                // 定期发送心跳
                _ = tokio::time::sleep(Duration::from_secs(config.heartbeat_interval_secs)) => {
                    let Some(snapshot) = heartbeat_snapshot.read().await.clone() else {
                        debug!("Skip heartbeat because no status snapshot is available yet");
                        continue;
                    };
                    let HeartbeatSnapshot { status, load } = snapshot;
                    let heartbeat = NodeToHub::Heartbeat {
                        message_id: uhorse_protocol::MessageId::new(),
                        node_id: node_id.clone(),
                        status,
                        load,
                        timestamp: Utc::now(),
                    };

                    if let Err(e) = Self::send_node_message(&mut ws_sender, &heartbeat).await {
                        warn!("Failed to send heartbeat: {}", e);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_node_message<S>(sender: &mut S, message: &NodeToHub) -> Result<(), String>
    where
        S: Sink<WsMessage> + Unpin,
        S::Error: std::fmt::Display,
    {
        let encoded = MessageCodec::encode_node_to_hub(message).map_err(|e| e.to_string())?;
        sender
            .send(WsMessage::Binary(encoded))
            .await
            .map_err(|e| e.to_string())
    }

    async fn recv_outbound_message(
        outbound_rx: &Arc<tokio::sync::Mutex<mpsc::Receiver<NodeToHub>>>,
    ) -> Option<NodeToHub> {
        let mut receiver = outbound_rx.lock().await;
        receiver.recv().await
    }

    /// 停止连接
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        *self.state.write().await = ConnectionState::Disconnected;
        info!("Connection stopped");
    }

    /// 获取连接状态
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        matches!(
            *self.state.read().await,
            ConnectionState::Connected { .. } | ConnectionState::Authenticated { .. }
        )
    }
}
