//! # WebSocket 处理器
//!
//! 实现 WebSocket 协议处理逻辑。

use uhorse_core::{
    ProtocolMessage, HandshakeRequest, HandshakeResponse,
    Event, ErrorDetail, Ping, Pong, ErrorCode,
    SessionId, Result, UHorseError,
};
use axum::{
    extract::{
        ws::{WebSocket, Message as AxumMessage},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error, instrument};

/// WebSocket 处理器状态
#[derive(Debug, Clone)]
pub struct WebSocketState {
    // TODO: 添加需要共享的状态
}

/// WebSocket 处理器
pub struct WebSocketHandler {
    state: Arc<WebSocketState>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        Self {
            state: Arc::new(WebSocketState {}),
        }
    }

    /// 处理 WebSocket 升级
    pub async fn handle_upgrade(
        ws: WebSocketUpgrade,
        State(state): State<Arc<WebSocketState>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| Self::handle_socket(socket, state))
    }

    /// 处理 WebSocket 连接
    #[instrument(skip(socket))]
    async fn handle_socket(mut socket: WebSocket, state: Arc<WebSocketState>) {
        info!("WebSocket connection established");

        // 连接状态
        let handshake_complete = Arc::new(RwLock::new(false));
        let _session_id = Arc::new(RwLock::new(None::<SessionId>));

        // 消息循环
        while let Some(msg) = socket.recv().await {
            match msg {
                Ok(AxumMessage::Text(text)) => {
                    debug!("Received text message: {}", text);

                    match Self::handle_message(&text, &state, &_session_id).await {
                        Ok(Some(response)) => {
                            if let Err(e) = socket.send(AxumMessage::Text(response)).await {
                                error!("Failed to send response: {}", e);
                                break;
                            }
                        }
                        Ok(None) => {
                            // 无需响应
                        }
                        Err(e) => {
                            warn!("Error handling message: {}", e);
                            let error_response = Self::create_error_response(&e);
                            if let Err(e) = socket.send(AxumMessage::Text(error_response)).await {
                                error!("Failed to send error response: {}", e);
                                break;
                            }
                        }
                    }
                }
                Ok(AxumMessage::Close(close_frame)) => {
                    info!("WebSocket close received: {:?}", close_frame);
                    break;
                }
                Ok(AxumMessage::Ping(data)) => {
                    if let Err(e) = socket.send(AxumMessage::Pong(data)).await {
                        error!("Failed to send pong: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        info!("WebSocket connection closed");
    }

    /// 处理消息
    async fn handle_message(
        text: &str,
        _state: &Arc<WebSocketState>,
        _session_id: &Arc<RwLock<Option<SessionId>>>,
    ) -> Result<Option<String>> {
        // 解析消息
        let msg: ProtocolMessage = serde_json::from_str(text)
            .map_err(|e| UHorseError::InvalidMessage(format!("Invalid JSON: {}", e)))?;

        match msg {
            ProtocolMessage::Handshake(req) => {
                Self::handle_handshake(req).await
            }
            ProtocolMessage::Ping(ping) => {
                let pong = Pong::new(ping.timestamp);
                let response = ProtocolMessage::Pong(pong);
                let json = serde_json::to_string(&response)?;
                Ok(Some(json))
            }
            _ => {
                // TODO: 实现其他消息类型
                debug!("Received message, type: {}", msg.type_name());
                Ok(None)
            }
        }
    }

    /// 处理握手
    async fn handle_handshake(req: HandshakeRequest) -> Result<Option<String>> {
        info!("Handshake request: version={:?}", req.version);

        // TODO: 验证版本兼容性
        // TODO: 验证认证令牌
        // TODO: 处理设备配对

        // 生成会话 ID
        let new_session_id = SessionId::new();

        let response = HandshakeResponse {
            server_version: semver::Version::new(0, 1, 0),
            session_id: new_session_id.as_str().to_string(),
            capabilities: uhorse_core::ServerCapabilities::default(),
            pairing_required: false,
            auth_status: uhorse_core::AuthStatus::Authenticated,
        };

        let msg = ProtocolMessage::HandshakeResponse(response);
        let json = serde_json::to_string(&msg)?;

        info!("Handshake completed for session: {}", new_session_id);

        Ok(Some(json))
    }

    /// 创建错误响应
    fn create_error_response(error: &UHorseError) -> String {
        let error_detail = ErrorDetail::new(error.code(), error.to_string());
        let response = uhorse_core::Response::err("error", error_detail);
        let msg = ProtocolMessage::Response(response);

        serde_json::to_string(&msg).unwrap_or_else(|_| r#"{"type":"response","data":{"error":"Internal error"}}"#.to_string())
    }
}

impl Default for WebSocketHandler {
    fn default() -> Self {
        Self::new()
    }
}
