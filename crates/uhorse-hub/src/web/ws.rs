//! WebSocket 端点处理
//!
//! 处理 Node 的 WebSocket 连接

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{HubToNode, MessageCodec, MessageId, NodeToHub};

use crate::{Hub, WebState};

/// WebSocket 升级处理器
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.hub.clone()))
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, hub: Arc<Hub>) {
    let (ws_sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(ws_sender));

    // 等待节点注册消息
    let registration = match wait_for_registration(&mut receiver).await {
        Some(reg) => reg,
        None => {
            warn!("Node disconnected without registration");
            return;
        }
    };

    let node_id = registration.node_id.clone();
    info!("Node {} connected via WebSocket", node_id);

    // 创建消息通道
    let (tx, mut rx) = tokio::sync::mpsc::channel::<HubToNode>(100);

    // 注册节点的消息发送器
    hub.message_router()
        .register_node_sender(node_id.clone(), tx.clone())
        .await;

    // 在 Hub 中注册节点
    if let Err(e) = hub
        .handle_node_connection(
            node_id.clone(),
            registration.name,
            registration.capabilities,
            registration.workspace,
            vec![], // tags - 协议暂不支持
        )
        .await
    {
        error!("Failed to register node {}: {}", node_id, e);
        hub.message_router().unregister_node_sender(&node_id).await;
        return;
    }

    // 发送心跳请求作为确认
    let ack = HubToNode::HeartbeatRequest {
        message_id: MessageId::new(),
        timestamp: Utc::now(),
    };

    if let Ok(msg) = MessageCodec::encode_hub_to_node(&ack) {
        let mut sender_guard = sender.lock().await;
        if sender_guard.send(Message::Binary(msg)).await.is_err() {
            error!("Failed to send ack message to node {}", node_id);
            drop(sender_guard);
            hub.message_router().unregister_node_sender(&node_id).await;
            let _ = hub.handle_node_disconnect(&node_id).await;
            return;
        }
    }

    // 发送任务 - 从 Hub 转发到 WebSocket
    let node_id_clone = node_id.clone();
    let sender_clone = sender.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(payload) = MessageCodec::encode_hub_to_node(&msg) {
                let mut sender_guard = sender_clone.lock().await;
                if sender_guard.send(Message::Binary(payload)).await.is_err() {
                    debug!("Node {} sender disconnected", node_id_clone);
                    break;
                }
            }
        }
    });

    // 接收任务 - 从 WebSocket 转发到 Hub
    let node_id_clone = node_id.clone();
    let hub_clone = hub.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => match MessageCodec::decode_node_to_hub(&data) {
                    Ok(node_msg) => {
                        if let Err(e) = hub_clone.handle_node_message(&node_id_clone, node_msg).await {
                            error!("Failed to handle node message: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to decode node message: {}", e);
                    }
                },
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<NodeToHub>(&text) {
                        Ok(node_msg) => {
                            if let Err(e) =
                                hub_clone.handle_node_message(&node_id_clone, node_msg).await
                            {
                                error!("Failed to handle node message: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse node message: {}", e);
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    debug!("Received ping from node {}", node_id_clone);
                    let mut sender_guard = sender.lock().await;
                    let _ = sender_guard.send(Message::Pong(data)).await;
                }
                Ok(Message::Close(_)) => {
                    info!("Node {} sent close frame", node_id_clone);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error from node {}: {}", node_id_clone, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待任一任务完成
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    // 清理
    info!("Node {} disconnected", node_id);
    hub.message_router().unregister_node_sender(&node_id).await;
    let _ = hub.handle_node_disconnect(&node_id).await;
}

/// 注册信息
struct Registration {
    node_id: uhorse_protocol::NodeId,
    name: String,
    capabilities: uhorse_protocol::NodeCapabilities,
    workspace: uhorse_protocol::WorkspaceInfo,
}

/// 等待节点注册
async fn wait_for_registration(
    receiver: &mut futures::stream::SplitStream<WebSocket>,
) -> Option<Registration> {
    // 设置超时
    let timeout = Duration::from_secs(30);

    match tokio::time::timeout(timeout, receiver.next()).await {
        Ok(Some(Ok(Message::Binary(data)))) => {
            match MessageCodec::decode_node_to_hub(&data) {
                Ok(NodeToHub::Register {
                    node_id,
                    name,
                    capabilities,
                    workspace,
                    ..
                }) => {
                    debug!("Node registration: {} ({})", name, node_id);
                    Some(Registration {
                        node_id,
                        name,
                        capabilities,
                        workspace,
                    })
                }
                Ok(other) => {
                    warn!(
                        "Expected Register message, got: {:?}",
                        std::mem::discriminant(&other)
                    );
                    None
                }
                Err(e) => {
                    error!("Failed to decode registration message: {}", e);
                    None
                }
            }
        }
        Ok(Some(Ok(Message::Text(text)))) => {
            match serde_json::from_str::<NodeToHub>(&text) {
                Ok(NodeToHub::Register {
                    node_id,
                    name,
                    capabilities,
                    workspace,
                    ..
                }) => {
                    debug!("Node registration: {} ({})", name, node_id);
                    Some(Registration {
                        node_id,
                        name,
                        capabilities,
                        workspace,
                    })
                }
                Ok(other) => {
                    warn!(
                        "Expected Register message, got: {:?}",
                        std::mem::discriminant(&other)
                    );
                    None
                }
                Err(e) => {
                    error!("Failed to parse registration message: {}", e);
                    None
                }
            }
        }
        Ok(Some(Ok(other))) => {
            warn!("Expected Text/Binary message for registration, got: {:?}", other);
            None
        }
        Ok(Some(Err(e))) => {
            error!("WebSocket error during registration: {}", e);
            None
        }
        Ok(None) => {
            warn!("Connection closed before registration");
            None
        }
        Err(_) => {
            warn!("Registration timeout");
            None
        }
    }
}
