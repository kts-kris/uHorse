//! # 钉钉通道适配器
//!
//! 完整实现钉钉机器人 API 集成，支持 Stream 模式。

use async_trait::async_trait;
use futures::{Sink, SinkExt, StreamExt};
use reqwest::{Client, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, instrument, warn};
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, MessageRole, Result, Session,
    UHorseError,
};

const STREAM_OPEN_URL: &str = "https://api.dingtalk.com/v1.0/gateway/connections/open";
const STREAM_CALLBACK_TOPIC: &str = "/v1.0/im/bot/messages/get";
const STREAM_RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const STREAM_CONTENT_TYPE: &str = "application/json";
const STREAM_UA: &str = "uhorse/4.0";

/// 钉钉访问令牌响应
#[derive(Debug, Deserialize)]
struct AccessTokenResult {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expireIn")]
    expires_in: i64,
}

/// 钉钉消息事件
#[derive(Debug, Deserialize)]
pub struct DingTalkEvent {
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "conversationType")]
    pub conversation_type: Option<String>,
    #[serde(rename = "conversationTitle")]
    pub conversation_title: Option<String>,
    #[serde(rename = "senderId")]
    pub sender_id: Option<String>,
    #[serde(rename = "senderNick")]
    pub sender_nick: Option<String>,
    #[serde(rename = "senderCorpId")]
    pub sender_corp_id: Option<String>,
    #[serde(rename = "senderStaffId")]
    pub sender_staff_id: Option<String>,
    #[serde(rename = "msgtype")]
    pub msg_type: Option<String>,
    pub text: Option<TextContent>,
    pub content: Option<serde_json::Value>,
    #[serde(rename = "sessionWebhook")]
    pub session_webhook: Option<String>,
    #[serde(rename = "sessionWebhookExpiredTime")]
    pub session_webhook_expired_time: Option<i64>,
    #[serde(rename = "robotCode")]
    pub robot_code: Option<String>,
    #[serde(rename = "createTime")]
    pub create_time: Option<i64>,
}

/// 文本内容
#[derive(Debug, Deserialize)]
pub struct TextContent {
    pub content: Option<String>,
}

/// Stream 消息
#[derive(Debug, Deserialize)]
pub struct StreamMessage {
    pub topic: String,
    pub data: String,
}

/// Stream 入站消息
#[derive(Debug, Clone)]
pub struct DingTalkInboundMessage {
    pub session: Session,
    pub message: Message,
    pub conversation_id: String,
    pub conversation_type: Option<String>,
    pub sender_user_id: Option<String>,
    pub sender_staff_id: Option<String>,
    pub sender_corp_id: Option<String>,
    pub session_webhook: Option<String>,
    pub session_webhook_expired_time: Option<i64>,
    pub robot_code: Option<String>,
}

/// Stream 建连请求
#[derive(Debug, Serialize)]
struct StreamConnectionRequest {
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(rename = "clientSecret")]
    client_secret: String,
    subscriptions: Vec<StreamSubscription>,
    ua: String,
}

/// Stream 订阅
#[derive(Debug, Serialize)]
struct StreamSubscription {
    topic: String,
    #[serde(rename = "type")]
    subscription_type: String,
}

/// Stream 建连响应
#[derive(Debug, Deserialize)]
struct StreamConnectionResponse {
    endpoint: String,
    ticket: String,
}

/// Stream 帧头
#[derive(Debug, Deserialize)]
struct StreamFrameHeaders {
    topic: Option<String>,
    #[serde(rename = "messageId")]
    message_id: Option<String>,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
}

/// Stream 帧
#[derive(Debug, Deserialize)]
struct StreamFrame {
    #[serde(rename = "type")]
    packet_type: String,
    headers: StreamFrameHeaders,
    #[serde(default)]
    data: serde_json::Value,
}

/// Stream ACK
#[derive(Debug, Serialize)]
struct StreamAck {
    code: i32,
    message: String,
    headers: StreamAckHeaders,
    data: String,
}

/// Stream ACK 头
#[derive(Debug, Serialize)]
struct StreamAckHeaders {
    #[serde(rename = "messageId")]
    message_id: String,
    #[serde(rename = "contentType")]
    content_type: String,
}

/// ping 数据
#[derive(Debug, Deserialize, Serialize)]
struct StreamPingData {
    opaque: String,
}

/// 发送消息请求
#[derive(Debug, Serialize)]
struct SendMessageRequest {
    #[serde(rename = "agent_id")]
    agent_id: String,
    #[serde(rename = "userid_list")]
    userid_list: String,
    msg: MessageBody,
}

/// 消息体
#[derive(Debug, Serialize)]
struct MessageBody {
    #[serde(rename = "msgtype")]
    msg_type: String,
    text: Option<TextBody>,
    #[serde(rename = "image")]
    image_body: Option<ImageBody>,
    #[serde(rename = "markdown")]
    markdown_body: Option<MarkdownBody>,
}

/// 文本消息体
#[derive(Debug, Serialize)]
struct TextBody {
    content: String,
}

/// 图片消息体
#[derive(Debug, Serialize)]
struct ImageBody {
    #[serde(rename = "mediaId")]
    media_id: String,
}

/// Markdown 消息体
#[derive(Debug, Serialize)]
struct MarkdownBody {
    title: String,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamAction {
    Continue,
    Reconnect,
}

/// 钉钉通道
#[derive(Debug, Clone)]
pub struct DingTalkChannel {
    app_key: String,
    app_secret: String,
    agent_id: u64,
    client: Client,
    running: Arc<RwLock<bool>>,
    access_token: Arc<RwLock<Option<String>>>,
    token_expires_at: Arc<RwLock<i64>>,
    incoming_tx: broadcast::Sender<DingTalkInboundMessage>,
}

impl DingTalkChannel {
    /// 创建新的钉钉通道
    pub fn new(app_key: String, app_secret: String, agent_id: u64) -> Self {
        let (incoming_tx, _) = broadcast::channel(128);

        Self {
            app_key,
            app_secret,
            agent_id,
            client: Client::new(),
            running: Arc::new(RwLock::new(false)),
            access_token: Arc::new(RwLock::new(None)),
            token_expires_at: Arc::new(RwLock::new(0)),
            incoming_tx,
        }
    }

    /// 订阅 Stream 入站消息
    pub fn subscribe_incoming(&self) -> broadcast::Receiver<DingTalkInboundMessage> {
        self.incoming_tx.subscribe()
    }

    /// 获取 app key
    pub fn app_key(&self) -> &str {
        &self.app_key
    }

    /// 获取 app secret
    pub fn app_secret(&self) -> &str {
        &self.app_secret
    }

    /// 获取 agent id
    pub fn agent_id(&self) -> u64 {
        self.agent_id
    }

    /// 获取访问令牌（带缓存）
    pub async fn get_access_token(&self) -> Result<String, ChannelError> {
        let now = chrono::Utc::now().timestamp();

        {
            let token = self.access_token.read().await;
            let expires_at = self.token_expires_at.read().await;
            if let Some(token) = token.as_ref() {
                if now < *expires_at - 300 {
                    return Ok(token.clone());
                }
            }
        }

        let url = "https://api.dingtalk.com/v1.0/oauth2/accessToken";

        #[derive(Serialize)]
        struct TokenRequest {
            #[serde(rename = "appKey")]
            app_key: String,
            #[serde(rename = "appSecret")]
            app_secret: String,
        }

        let response = self
            .client
            .post(url)
            .json(&TokenRequest {
                app_key: self.app_key.clone(),
                app_secret: self.app_secret.clone(),
            })
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::ConfigError(format!(
                "Failed to get access token: {}",
                error_text
            )));
        }

        let result: AccessTokenResult = response
            .json()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("JSON error: {}", e)))?;

        let expires_at = now + result.expires_in;
        *self.access_token.write().await = Some(result.access_token.clone());
        *self.token_expires_at.write().await = expires_at;

        info!(
            "DingTalk access token obtained, expires in {} seconds",
            result.expires_in
        );
        Ok(result.access_token)
    }

    /// 处理钉钉事件回调 (原始 JSON)
    pub async fn handle_event_raw(
        &self,
        event_json: &str,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        debug!("Handling DingTalk event (raw)");

        let event: DingTalkEvent = serde_json::from_str(event_json).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to parse event JSON: {}", e))
        })?;

        Ok(self
            .handle_event_with_metadata(&event)
            .await?
            .map(|incoming| (incoming.session, incoming.message)))
    }

    /// 处理钉钉事件
    pub async fn handle_event(
        &self,
        event: &DingTalkEvent,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        Ok(self
            .handle_event_with_metadata(event)
            .await?
            .map(|incoming| (incoming.session, incoming.message)))
    }

    /// 处理钉钉事件并返回回传路由信息
    pub async fn handle_event_with_metadata(
        &self,
        event: &DingTalkEvent,
    ) -> Result<Option<DingTalkInboundMessage>, ChannelError> {
        let conversation_id = match &event.conversation_id {
            Some(id) => id.clone(),
            None => {
                debug!("No conversation_id in event");
                return Ok(None);
            }
        };

        let session = Session::new(ChannelType::DingTalk, conversation_id.clone());
        let message_content = self.extract_content(event);

        debug!(
            "Processed DingTalk message: conversation_id={}, sender_id={}, content={:?}",
            conversation_id,
            event.sender_id.as_deref().unwrap_or("unknown"),
            message_content
        );

        let message = Message::new(
            session.id.clone(),
            MessageRole::User,
            message_content,
            event.create_time.unwrap_or(0) as u64,
        );

        Ok(Some(DingTalkInboundMessage {
            session,
            message,
            conversation_id,
            conversation_type: event.conversation_type.clone(),
            sender_user_id: event.sender_id.clone(),
            sender_staff_id: event.sender_staff_id.clone(),
            sender_corp_id: event.sender_corp_id.clone(),
            session_webhook: event.session_webhook.clone(),
            session_webhook_expired_time: event.session_webhook_expired_time,
            robot_code: event.robot_code.clone(),
        }))
    }

    /// 处理 Stream 消息
    pub async fn handle_stream_message(
        &self,
        stream_msg: &StreamMessage,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        Ok(self
            .handle_stream_message_with_metadata(stream_msg)
            .await?
            .map(|incoming| (incoming.session, incoming.message)))
    }

    /// 处理 Stream 消息并返回回传路由信息
    pub async fn handle_stream_message_with_metadata(
        &self,
        stream_msg: &StreamMessage,
    ) -> Result<Option<DingTalkInboundMessage>, ChannelError> {
        debug!(
            "Handling DingTalk stream message: topic={}",
            stream_msg.topic
        );

        let event: DingTalkEvent = serde_json::from_str(&stream_msg.data).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to parse stream data: {}", e))
        })?;

        self.handle_event_with_metadata(&event).await
    }

    /// 提取消息内容
    fn extract_content(&self, event: &DingTalkEvent) -> MessageContent {
        if let Some(text) = &event.text {
            if let Some(content) = &text.content {
                return MessageContent::Text(content.clone());
            }
        }

        if let Some(content) = &event.content {
            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                return MessageContent::Text(text.to_string());
            }

            if let Some(image_key) = content.get("imageKey").and_then(|v| v.as_str()) {
                return MessageContent::Image {
                    url: format!("dingtalk://image?key={}", image_key),
                    caption: content
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                };
            }

            if let Some(audio_key) = content.get("audioKey").and_then(|v| v.as_str()) {
                let duration = content
                    .get("duration")
                    .and_then(|v| v.as_u64())
                    .map(|d| d as u32);
                return MessageContent::Audio {
                    url: format!("dingtalk://audio?key={}", audio_key),
                    duration,
                };
            }

            if let Some(file_key) = content.get("fileKey").and_then(|v| v.as_str()) {
                let file_name = content
                    .get("fileName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("文件");
                return MessageContent::Text(format!("[{}] {}", file_name, file_key));
            }

            if let Some(markdown) = content.get("markdown").and_then(|v| v.as_str()) {
                return MessageContent::Text(markdown.to_string());
            }
        }

        MessageContent::Text("[不支持的消息类型]".to_string())
    }

    async fn stream_loop(self) {
        while self.is_running_async().await {
            match self.connect_stream().await {
                Ok(ws_stream) => {
                    info!("DingTalk stream connected");
                    if let Err(error) = self.run_stream_connection(ws_stream).await {
                        warn!("DingTalk stream connection ended: {}", error);
                    }
                }
                Err(error) => {
                    warn!("Failed to connect DingTalk stream: {}", error);
                }
            }

            if !self.is_running_async().await {
                break;
            }

            tokio::time::sleep(STREAM_RECONNECT_INTERVAL).await;
        }

        info!("DingTalk stream loop stopped");
    }

    async fn connect_stream(
        &self,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        ChannelError,
    > {
        let response = self
            .client
            .post(STREAM_OPEN_URL)
            .json(&StreamConnectionRequest {
                client_id: self.app_key.clone(),
                client_secret: self.app_secret.clone(),
                subscriptions: vec![StreamSubscription {
                    topic: STREAM_CALLBACK_TOPIC.to_string(),
                    subscription_type: "CALLBACK".to_string(),
                }],
                ua: STREAM_UA.to_string(),
            })
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("Open stream HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::ConnectionError(format!(
                "Open stream failed: {}",
                error_text
            )));
        }

        let connection: StreamConnectionResponse = response
            .json()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("Open stream JSON error: {}", e)))?;

        let mut url = Url::parse(&connection.endpoint)
            .map_err(|e| ChannelError::ConfigError(format!("Invalid stream endpoint: {}", e)))?;
        url.query_pairs_mut()
            .append_pair("ticket", &connection.ticket);

        let (ws_stream, _) = connect_async(url.as_str()).await.map_err(|e| {
            ChannelError::ConnectionError(format!("WebSocket connect error: {}", e))
        })?;

        Ok(ws_stream)
    }

    async fn run_stream_connection(
        &self,
        ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<(), ChannelError> {
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        while self.is_running_async().await {
            match ws_receiver.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if self
                        .handle_stream_text_frame(&mut ws_sender, text.as_ref())
                        .await?
                        == StreamAction::Reconnect
                    {
                        break;
                    }
                }
                Some(Ok(WsMessage::Binary(data))) => {
                    let text = String::from_utf8(data.to_vec()).map_err(|e| {
                        ChannelError::InvalidResponse(format!(
                            "Invalid binary stream payload: {}",
                            e
                        ))
                    })?;
                    if self.handle_stream_text_frame(&mut ws_sender, &text).await?
                        == StreamAction::Reconnect
                    {
                        break;
                    }
                }
                Some(Ok(WsMessage::Ping(data))) => {
                    ws_sender.send(WsMessage::Pong(data)).await.map_err(|e| {
                        ChannelError::ConnectionError(format!("Pong send error: {}", e))
                    })?;
                }
                Some(Ok(WsMessage::Pong(_))) => {}
                Some(Ok(WsMessage::Close(frame))) => {
                    info!("DingTalk stream closed by server: {:?}", frame);
                    break;
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    return Err(ChannelError::ConnectionError(format!(
                        "Stream receive error: {}",
                        error
                    )));
                }
                None => {
                    return Err(ChannelError::ConnectionLost);
                }
            }
        }

        Ok(())
    }

    async fn handle_stream_text_frame<S>(
        &self,
        ws_sender: &mut S,
        payload: &str,
    ) -> Result<StreamAction, ChannelError>
    where
        S: Sink<WsMessage> + Unpin,
        S::Error: std::fmt::Display,
    {
        let frame: StreamFrame = serde_json::from_str(payload).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to parse stream frame: {}", e))
        })?;

        let topic = frame.headers.topic.as_deref().unwrap_or_default();

        match frame.packet_type.as_str() {
            "SYSTEM" => match topic {
                "ping" => {
                    if let Some(ack) = Self::build_ping_ack(&frame)? {
                        Self::send_ws_text(ws_sender, ack).await?;
                    }
                    Ok(StreamAction::Continue)
                }
                "disconnect" => {
                    info!("DingTalk stream requested reconnect");
                    Ok(StreamAction::Reconnect)
                }
                _ => {
                    warn!("Ignoring DingTalk system frame with topic: {}", topic);
                    Ok(StreamAction::Continue)
                }
            },
            "CALLBACK" => {
                let stream_msg = StreamMessage {
                    topic: topic.to_string(),
                    data: Self::frame_data_to_string(&frame.data),
                };

                match self.handle_stream_message_with_metadata(&stream_msg).await {
                    Ok(Some(incoming)) => {
                        if let Err(error) = self.incoming_tx.send(incoming) {
                            warn!("Dropped DingTalk inbound message: {}", error);
                        }
                    }
                    Ok(None) => {
                        debug!("Ignored DingTalk callback without actionable message");
                    }
                    Err(error) => {
                        warn!("Failed to process DingTalk callback: {}", error);
                        if let Some(ack) =
                            Self::build_callback_error_ack(&frame, &error.to_string())?
                        {
                            Self::send_ws_text(ws_sender, ack).await?;
                        }
                        return Ok(StreamAction::Continue);
                    }
                }

                if let Some(ack) = Self::build_callback_ack(&frame)? {
                    Self::send_ws_text(ws_sender, ack).await?;
                }
                Ok(StreamAction::Continue)
            }
            "EVENT" => {
                debug!("Ignoring DingTalk event topic: {}", topic);
                if let Some(ack) = Self::build_event_ack(&frame, true, "success")? {
                    Self::send_ws_text(ws_sender, ack).await?;
                }
                Ok(StreamAction::Continue)
            }
            other => {
                warn!(
                    "Ignoring unsupported DingTalk stream packet type: {}",
                    other
                );
                Ok(StreamAction::Continue)
            }
        }
    }

    fn frame_data_to_string(data: &serde_json::Value) -> String {
        match data {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        }
    }

    fn parse_frame_data<T: DeserializeOwned>(data: &serde_json::Value) -> Result<T, ChannelError> {
        match data {
            serde_json::Value::String(text) => serde_json::from_str(text).map_err(|e| {
                ChannelError::InvalidResponse(format!("Failed to parse frame data: {}", e))
            }),
            other => serde_json::from_value(other.clone()).map_err(|e| {
                ChannelError::InvalidResponse(format!("Failed to parse frame value: {}", e))
            }),
        }
    }

    fn build_ack(
        frame: &StreamFrame,
        code: i32,
        message: impl Into<String>,
        data: String,
    ) -> Result<Option<String>, ChannelError> {
        let Some(message_id) = frame.headers.message_id.as_ref() else {
            return Ok(None);
        };

        serde_json::to_string(&StreamAck {
            code,
            message: message.into(),
            headers: StreamAckHeaders {
                message_id: message_id.clone(),
                content_type: frame
                    .headers
                    .content_type
                    .clone()
                    .unwrap_or_else(|| STREAM_CONTENT_TYPE.to_string()),
            },
            data,
        })
        .map(Some)
        .map_err(|e| ChannelError::InvalidResponse(format!("Failed to serialize ACK: {}", e)))
    }

    fn build_ping_ack(frame: &StreamFrame) -> Result<Option<String>, ChannelError> {
        let ping_data: StreamPingData = Self::parse_frame_data(&frame.data)?;
        let data = serde_json::to_string(&ping_data).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to serialize ping ACK: {}", e))
        })?;
        Self::build_ack(frame, 200, "OK", data)
    }

    fn build_callback_ack(frame: &StreamFrame) -> Result<Option<String>, ChannelError> {
        Self::build_ack(
            frame,
            200,
            "OK",
            serde_json::json!({ "response": serde_json::Value::Null }).to_string(),
        )
    }

    fn build_callback_error_ack(
        frame: &StreamFrame,
        error_message: &str,
    ) -> Result<Option<String>, ChannelError> {
        Self::build_ack(
            frame,
            500,
            error_message.to_string(),
            serde_json::json!({ "response": serde_json::Value::Null }).to_string(),
        )
    }

    fn build_event_ack(
        frame: &StreamFrame,
        success: bool,
        message: &str,
    ) -> Result<Option<String>, ChannelError> {
        let code = if success { 200 } else { 500 };
        let status = if success { "SUCCESS" } else { "LATER" };
        Self::build_ack(
            frame,
            code,
            if success { "OK" } else { message }.to_string(),
            serde_json::json!({
                "status": status,
                "message": message,
            })
            .to_string(),
        )
    }

    async fn send_ws_text<S>(ws_sender: &mut S, payload: String) -> Result<(), ChannelError>
    where
        S: Sink<WsMessage> + Unpin,
        S::Error: std::fmt::Display,
    {
        ws_sender
            .send(WsMessage::Text(payload.into()))
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("WebSocket send error: {}", e)))
    }

    async fn is_running_async(&self) -> bool {
        *self.running.read().await
    }

    /// 通过 sessionWebhook 原路回复文本消息
    pub async fn reply_via_session_webhook(
        &self,
        session_webhook: &str,
        text: &str,
        at_user_ids: &[String],
    ) -> Result<(), ChannelError> {
        #[derive(Serialize)]
        struct SessionAtBody {
            #[serde(rename = "atUserIds")]
            at_user_ids: Vec<String>,
            #[serde(rename = "isAtAll")]
            is_at_all: bool,
        }

        #[derive(Serialize)]
        struct SessionTextRequest {
            at: SessionAtBody,
            text: TextBody,
            #[serde(rename = "msgtype")]
            msg_type: String,
        }

        let access_token = self.get_access_token().await?;
        let request = SessionTextRequest {
            at: SessionAtBody {
                at_user_ids: at_user_ids.to_vec(),
                is_at_all: false,
            },
            text: TextBody {
                content: text.to_string(),
            },
            msg_type: "text".to_string(),
        };

        let response = self
            .client
            .post(session_webhook)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("DingTalk session webhook error: {}", error_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送文本消息
    pub async fn send_text(&self, user_id: &str, text: &str) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend";

        let request = SendMessageRequest {
            agent_id: self.agent_id.to_string(),
            userid_list: user_id.to_string(),
            msg: MessageBody {
                msg_type: "text".to_string(),
                text: Some(TextBody {
                    content: text.to_string(),
                }),
                image_body: None,
                markdown_body: None,
            },
        };

        let response = self
            .client
            .post(url)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("DingTalk API error: {}", error_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送 Markdown 消息
    pub async fn send_markdown(
        &self,
        user_id: &str,
        title: &str,
        markdown: &str,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend";

        let request = SendMessageRequest {
            agent_id: self.agent_id.to_string(),
            userid_list: user_id.to_string(),
            msg: MessageBody {
                msg_type: "markdown".to_string(),
                text: None,
                image_body: None,
                markdown_body: Some(MarkdownBody {
                    title: title.to_string(),
                    text: markdown.to_string(),
                }),
            },
        };

        let response = self
            .client
            .post(url)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送群消息
    pub async fn send_group_message(
        &self,
        conversation_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/robot/groupMessages/send";

        match message {
            MessageContent::Text(text) => {
                #[derive(Serialize)]
                struct GroupTextRequest {
                    #[serde(rename = "conversationId")]
                    conversation_id: String,
                    msg: GroupMessageBody,
                }

                #[derive(Serialize)]
                struct GroupMessageBody {
                    #[serde(rename = "msgtype")]
                    msg_type: String,
                    text: TextBody,
                }

                let request = GroupTextRequest {
                    conversation_id: conversation_id.to_string(),
                    msg: GroupMessageBody {
                        msg_type: "text".to_string(),
                        text: TextBody {
                            content: text.clone(),
                        },
                    },
                };

                let response = self
                    .client
                    .post(url)
                    .header("x-acs-dingtalk-access-token", access_token.clone())
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

                if !response.status().is_success() {
                    let error_text = response.text().await.unwrap_or_default();
                    return Err(ChannelError::SendFailed(format!(
                        "API error: {}",
                        error_text
                    )));
                }
            }
            _ => {
                return Err(ChannelError::SendFailed(
                    "Only text messages are supported for group messages".to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for DingTalkChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::DingTalk
    }

    #[instrument(skip(self, message))]
    async fn send_message(
        &self,
        user_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        debug!("Sending DingTalk message to {}: {:?}", user_id, message);

        match message {
            MessageContent::Text(text) => {
                self.send_text(user_id, text).await?;
            }
            MessageContent::Image { url, caption } => {
                let text = format!(
                    "[图片] {}{}",
                    url,
                    caption
                        .as_ref()
                        .map(|c| format!(" - {}", c))
                        .unwrap_or_default()
                );
                self.send_text(user_id, &text).await?;
            }
            MessageContent::Audio { url, duration } => {
                let text = format!("[音频] {} ({}秒)", url, duration.unwrap_or(0));
                self.send_text(user_id, &text).await?;
            }
            MessageContent::Structured(data) => {
                let json = serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| "Invalid JSON".to_string());
                self.send_markdown(user_id, "数据", &format!("```\n{}\n```", json))
                    .await?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_webhook(
        &self,
        _payload: &[u8],
        signature: Option<&str>,
    ) -> Result<bool, ChannelError> {
        debug!("Verifying DingTalk webhook");

        if let Some(sig) = signature {
            debug!("Signature provided: {}", sig);
        }

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        if *self.running.read().await {
            return Ok(());
        }

        info!("Starting DingTalk channel in Stream mode");

        match self.get_access_token().await {
            Ok(token) => {
                info!("DingTalk channel connected, token obtained");
                debug!("Access token: {}...", &token[..20.min(token.len())]);
            }
            Err(e) => {
                error!("Failed to connect to DingTalk: {}", e);
                return Err(UHorseError::ChannelError(e));
            }
        }

        *self.running.write().await = true;

        let channel = self.clone();
        tokio::spawn(async move {
            channel.stream_loop().await;
        });

        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping DingTalk channel");
        *self.running.write().await = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.blocking_read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dingtalk_channel_creation() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789);

        assert_eq!(channel.app_key(), "test_key");
        assert_eq!(channel.app_secret(), "test_secret");
        assert_eq!(channel.agent_id(), 123456789);
        assert_eq!(channel.channel_type(), ChannelType::DingTalk);
    }

    #[test]
    fn test_extract_text_content() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789);

        let event = DingTalkEvent {
            conversation_id: Some("conv_123".to_string()),
            conversation_type: Some("1".to_string()),
            conversation_title: None,
            sender_id: Some("user_456".to_string()),
            sender_nick: None,
            sender_corp_id: None,
            sender_staff_id: None,
            msg_type: Some("text".to_string()),
            text: Some(TextContent {
                content: Some("Hello".to_string()),
            }),
            content: None,
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: None,
            create_time: Some(1234567890),
        };

        let content = channel.extract_content(&event);
        assert!(matches!(content, MessageContent::Text(t) if t == "Hello"));
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{
            "conversationId": "conv_123",
            "conversationType": "1",
            "senderId": "user_456",
            "msgtype": "text",
            "text": {
                "content": "Hello World"
            },
            "createTime": 1234567890
        }"#;

        let event: DingTalkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.conversation_id, Some("conv_123".to_string()));
        assert_eq!(event.sender_id, Some("user_456".to_string()));
        assert_eq!(
            event.text.as_ref().unwrap().content,
            Some("Hello World".to_string())
        );
    }
}
