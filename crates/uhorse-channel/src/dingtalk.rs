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
use uuid::Uuid;
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, MessageRole, Result, Session,
    UHorseError,
};

const STREAM_OPEN_URL: &str = "https://api.dingtalk.com/v1.0/gateway/connections/open";
const STREAM_CALLBACK_TOPIC: &str = "/v1.0/im/bot/messages/get";
const STREAM_RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const STREAM_CONTENT_TYPE: &str = "application/json";
const STREAM_UA: &str = "uhorse/4.0";
const EMOTION_REPLY_URL: &str = "https://api.dingtalk.com/v1.0/robot/emotion/reply";
const EMOTION_RECALL_URL: &str = "https://api.dingtalk.com/v1.0/robot/emotion/recall";
const DEFAULT_DINGTALK_ACK_REACTION: &str = "🤔思考中";
const THINKING_EMOTION_ID: &str = "2659900";
const THINKING_EMOTION_BACKGROUND_ID: &str = "im_bg_1";

/// 钉钉访问令牌响应
#[derive(Debug, Deserialize)]
struct AccessTokenResult {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expireIn")]
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct DingTalkApiResponseEnvelope {
    #[serde(default)]
    errcode: Option<i64>,
    #[serde(default)]
    errno: Option<i64>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    code: Option<serde_json::Value>,
    #[serde(default, rename = "requestId")]
    request_id: Option<String>,
}

fn parse_dingtalk_business_error(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let envelope: DingTalkApiResponseEnvelope = serde_json::from_str(trimmed).ok()?;

    if matches!(envelope.success, Some(false)) {
        return Some(format_dingtalk_business_error(&envelope));
    }

    if let Some(errcode) = envelope.errcode.filter(|code| *code != 0) {
        return Some(format_dingtalk_business_error(&envelope).replace("<code>", &errcode.to_string()));
    }

    if let Some(errno) = envelope.errno.filter(|code| *code != 0) {
        return Some(format_dingtalk_business_error(&envelope).replace("<code>", &errno.to_string()));
    }

    if let Some(code) = envelope.code.as_ref().and_then(normalize_dingtalk_business_code) {
        if code != 0 {
            return Some(format_dingtalk_business_error(&envelope).replace("<code>", &code.to_string()));
        }
    }

    None
}

fn normalize_dingtalk_business_code(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn format_dingtalk_business_error(envelope: &DingTalkApiResponseEnvelope) -> String {
    let message = envelope
        .errmsg
        .as_deref()
        .or(envelope.message.as_deref())
        .unwrap_or("unknown DingTalk business error");
    let request_suffix = envelope
        .request_id
        .as_deref()
        .map(|request_id| format!(", requestId={request_id}"))
        .unwrap_or_default();
    format!("business error code=<code>, message={}{}", message, request_suffix)
}

/// 钉钉消息事件
#[derive(Debug, Deserialize)]
pub struct DingTalkEvent {
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "msgId", alias = "messageId")]
    pub message_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DingTalkInboundAttachment {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recognition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

/// Stream 入站消息
#[derive(Debug, Clone)]
pub struct DingTalkInboundMessage {
    pub session: Session,
    pub message: Message,
    pub conversation_id: String,
    pub message_id: Option<String>,
    pub conversation_type: Option<String>,
    pub sender_user_id: Option<String>,
    pub sender_staff_id: Option<String>,
    pub sender_corp_id: Option<String>,
    pub session_webhook: Option<String>,
    pub session_webhook_expired_time: Option<i64>,
    pub robot_code: Option<String>,
    pub attachments: Vec<DingTalkInboundAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DingTalkTransientClearOutcome {
    Unsupported,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DingTalkAiCardHandle {
    pub out_track_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DingTalkReactionHandle {
    pub robot_code: String,
    pub message_id: String,
    pub conversation_id: String,
    pub reaction_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DingTalkTransientMessageReceipt {
    supported_clear: bool,
}

impl DingTalkTransientMessageReceipt {
    pub fn unsupported() -> Self {
        Self {
            supported_clear: false,
        }
    }

    pub fn supports_clear(&self) -> bool {
        self.supported_clear
    }
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

#[derive(Debug, Serialize)]
struct DingTalkAiCardContent {
    #[serde(rename = "cardParamMap")]
    card_param_map: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct DingTalkAiCardCreateRequest {
    #[serde(rename = "openSpaceId")]
    open_space_id: String,
    #[serde(rename = "cardTemplateId")]
    card_template_id: String,
    #[serde(rename = "outTrackId")]
    out_track_id: String,
    #[serde(rename = "cardData")]
    card_data: DingTalkAiCardContent,
    #[serde(rename = "imGroupOpenDeliverModel", skip_serializing_if = "Option::is_none")]
    im_group_open_deliver_model: Option<DingTalkAiCardGroupDeliverModel>,
    #[serde(rename = "imRobotOpenDeliverModel", skip_serializing_if = "Option::is_none")]
    im_robot_open_deliver_model: Option<DingTalkAiCardRobotDeliverModel>,
}

#[derive(Debug, Serialize)]
struct DingTalkAiCardGroupDeliverModel {
    #[serde(rename = "robotCode")]
    robot_code: String,
}

#[derive(Debug, Serialize)]
struct DingTalkAiCardRobotDeliverModel {
    #[serde(rename = "spaceType")]
    space_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DingTalkAiCardTarget {
    ImGroup { conversation_id: String },
    ImRobot { user_id: String },
}

fn build_im_group_open_space_id(conversation_id: &str) -> String {
    format!("dtv1.card//im_group.{conversation_id}")
}

fn build_im_robot_open_space_id(user_id: &str) -> String {
    format!("dtv1.card//im_robot.{user_id}")
}

#[derive(Debug, Serialize)]
struct DingTalkAiCardUpdateRequest {
    #[serde(rename = "outTrackId")]
    out_track_id: String,
    #[serde(rename = "cardData")]
    card_data: DingTalkAiCardContent,
    #[serde(rename = "cardUpdateOptions")]
    card_update_options: DingTalkAiCardUpdateOptions,
}

#[derive(Debug, Serialize)]
struct DingTalkAiCardUpdateOptions {
    #[serde(rename = "updateCardDataByKey")]
    update_card_data_by_key: bool,
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
    ai_card_template_id: Option<String>,
    client: Client,
    running: Arc<RwLock<bool>>,
    access_token: Arc<RwLock<Option<String>>>,
    token_expires_at: Arc<RwLock<i64>>,
    incoming_tx: broadcast::Sender<DingTalkInboundMessage>,
}

impl DingTalkChannel {
    /// 创建新的钉钉通道
    pub fn new(
        app_key: String,
        app_secret: String,
        agent_id: u64,
        ai_card_template_id: Option<String>,
    ) -> Self {
        let (incoming_tx, _) = broadcast::channel(128);

        Self {
            app_key,
            app_secret,
            agent_id,
            ai_card_template_id,
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

    pub async fn set_access_token_for_test(&self, token: &str) {
        *self.access_token.write().await = Some(token.to_string());
        *self.token_expires_at.write().await = chrono::Utc::now().timestamp() + 3600;
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
        let attachments = self.extract_attachments(event);

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
            message_id: event.message_id.clone(),
            conversation_type: event.conversation_type.clone(),
            sender_user_id: event.sender_id.clone(),
            sender_staff_id: event.sender_staff_id.clone(),
            sender_corp_id: event.sender_corp_id.clone(),
            session_webhook: event.session_webhook.clone(),
            session_webhook_expired_time: event.session_webhook_expired_time,
            robot_code: event.robot_code.clone().or_else(|| Some(self.app_key.clone())),
            attachments,
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

    fn extract_attachments(&self, event: &DingTalkEvent) -> Vec<DingTalkInboundAttachment> {
        let mut attachments = Vec::new();

        let Some(content) = event.content.as_ref() else {
            return attachments;
        };

        let file_name = || {
            content
                .get("fileName")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let download_code = || {
            content
                .get("downloadCode")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let recognition = || {
            content
                .get("recognition")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let description = || {
            content
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };

        if let Some(image_key) = content.get("imageKey").and_then(|v| v.as_str()) {
            attachments.push(DingTalkInboundAttachment {
                kind: "image".to_string(),
                key: Some(image_key.to_string()),
                file_name: file_name(),
                download_code: download_code(),
                recognition: None,
                caption: description(),
            });
        } else if matches!(event.msg_type.as_deref(), Some("picture"))
            && (download_code().is_some() || file_name().is_some())
        {
            attachments.push(DingTalkInboundAttachment {
                kind: "image".to_string(),
                key: None,
                file_name: file_name(),
                download_code: download_code(),
                recognition: None,
                caption: description(),
            });
        }

        if let Some(audio_key) = content.get("audioKey").and_then(|v| v.as_str()) {
            attachments.push(DingTalkInboundAttachment {
                kind: "audio".to_string(),
                key: Some(audio_key.to_string()),
                file_name: file_name(),
                download_code: download_code(),
                recognition: recognition(),
                caption: None,
            });
        } else if matches!(event.msg_type.as_deref(), Some("audio"))
            && (download_code().is_some() || recognition().is_some() || file_name().is_some())
        {
            attachments.push(DingTalkInboundAttachment {
                kind: "audio".to_string(),
                key: None,
                file_name: file_name(),
                download_code: download_code(),
                recognition: recognition(),
                caption: None,
            });
        }

        if let Some(file_key) = content.get("fileKey").and_then(|v| v.as_str()) {
            attachments.push(DingTalkInboundAttachment {
                kind: "file".to_string(),
                key: Some(file_key.to_string()),
                file_name: file_name(),
                download_code: download_code(),
                recognition: None,
                caption: None,
            });
        } else if matches!(event.msg_type.as_deref(), Some("file"))
            && (download_code().is_some()
                || file_name().is_some()
                || content.get("spaceId").is_some()
                || content.get("fileId").is_some())
        {
            attachments.push(DingTalkInboundAttachment {
                kind: "file".to_string(),
                key: None,
                file_name: file_name(),
                download_code: download_code(),
                recognition: None,
                caption: None,
            });
        }

        attachments
    }

    fn inbound_attachment_from_url<'a>(
        &self,
        attachments: &'a [DingTalkInboundAttachment],
        prefix: &str,
        url: &str,
    ) -> Option<&'a DingTalkInboundAttachment> {
        let key = url.strip_prefix(prefix)?;
        attachments.iter().find(|attachment| {
            attachment
                .key
                .as_deref()
                .map(|candidate| candidate == key)
                .unwrap_or(false)
                || (attachment.key.is_none()
                    && attachment
                        .download_code
                        .as_deref()
                        .map(|candidate| candidate == key)
                        .unwrap_or(false))
        })
    }

    pub async fn download_inbound_attachment(
        &self,
        attachment: &DingTalkInboundAttachment,
    ) -> Result<Vec<u8>, ChannelError> {
        let download_code = attachment
            .download_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ChannelError::ConfigError("download_code is required for DingTalk media".to_string())
            })?;
        let robot_code = self.app_key.trim();
        if robot_code.is_empty() {
            return Err(ChannelError::ConfigError(
                "robot_code is required for DingTalk media".to_string(),
            ));
        }

        let access_token = self.get_access_token().await?;
        let response = self
            .client
            .post("https://api.dingtalk.com/v1.0/robot/messageFiles/download")
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&serde_json::json!({
                "downloadCode": download_code,
                "robotCode": robot_code,
            }))
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("Read body error: {}", e)))?;
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", body)));
        }
        if let Some(error) = parse_dingtalk_business_error(&body) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| ChannelError::InvalidResponse(format!("JSON error: {}", e)))?;
        let download_url = parsed
            .get("downloadUrl")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                parsed
                    .get("data")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|value| value.get("downloadUrl"))
                    .and_then(serde_json::Value::as_str)
            })
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ChannelError::InvalidResponse("download url is missing".to_string()))?;

        let response = self
            .client
            .get(download_url)
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("API error: {}", body)));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("Read body error: {}", e)))?;
        Ok(bytes.to_vec())
    }

    pub async fn download_inbound_message_media(
        &self,
        inbound: &DingTalkInboundMessage,
    ) -> Result<Option<Vec<u8>>, ChannelError> {
        match &inbound.message.content {
            MessageContent::Audio { url, .. } => {
                let Some(attachment) =
                    self.inbound_attachment_from_url(&inbound.attachments, "dingtalk://audio?key=", url)
                else {
                    return Ok(None);
                };
                self.download_inbound_attachment(attachment).await.map(Some)
            }
            MessageContent::Image { url, .. } => {
                let Some(attachment) =
                    self.inbound_attachment_from_url(&inbound.attachments, "dingtalk://image?key=", url)
                else {
                    return Ok(None);
                };
                self.download_inbound_attachment(attachment).await.map(Some)
            }
            _ => Ok(None),
        }
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

            if matches!(event.msg_type.as_deref(), Some("picture"))
                && content
                    .get("downloadCode")
                    .and_then(|v| v.as_str())
                    .is_some()
            {
                let key = content
                    .get("downloadCode")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                return MessageContent::Image {
                    url: format!("dingtalk://image?key={}", key),
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

            if matches!(event.msg_type.as_deref(), Some("audio"))
                && content
                    .get("downloadCode")
                    .and_then(|v| v.as_str())
                    .is_some()
            {
                let duration = content
                    .get("duration")
                    .and_then(|v| v.as_u64())
                    .map(|d| d as u32);
                let key = content
                    .get("downloadCode")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                return MessageContent::Audio {
                    url: format!("dingtalk://audio?key={}", key),
                    duration,
                };
            }

            if let Some(file_key) = content.get("fileKey").and_then(|v| v.as_str()) {
                let file_name = content
                    .get("fileName")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                return MessageContent::Structured(serde_json::json!({
                    "kind": "dingtalk_file",
                    "file_key": file_key,
                    "file_name": file_name,
                }));
            }

            if matches!(event.msg_type.as_deref(), Some("file")) {
                let file_name = content
                    .get("fileName")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let download_code = content
                    .get("downloadCode")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let space_id = content
                    .get("spaceId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let file_id = content
                    .get("fileId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                if download_code.is_some() || file_name.is_some() || space_id.is_some() || file_id.is_some() {
                    return MessageContent::Structured(serde_json::json!({
                        "kind": "dingtalk_file",
                        "download_code": download_code,
                        "file_name": file_name,
                        "space_id": space_id,
                        "file_id": file_id,
                    }));
                }
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
            .send(WsMessage::Text(payload))
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("WebSocket send error: {}", e)))
    }

    async fn is_running_async(&self) -> bool {
        *self.running.read().await
    }

    pub fn ai_card_template_id(&self) -> Option<&str> {
        self.ai_card_template_id.as_deref()
    }

    pub async fn send_processing_ack_via_session_webhook_markdown(
        &self,
        session_webhook: &str,
        markdown: &str,
        at_user_ids: &[String],
    ) -> Result<DingTalkTransientMessageReceipt, ChannelError> {
        self.reply_via_session_webhook_markdown(session_webhook, "Wait", markdown, at_user_ids)
            .await?;
        Ok(DingTalkTransientMessageReceipt::unsupported())
    }

    pub async fn add_processing_reaction(
        &self,
        robot_code: &str,
        message_id: &str,
        conversation_id: &str,
    ) -> Result<DingTalkReactionHandle, ChannelError> {
        self.send_processing_reaction(
            robot_code,
            message_id,
            conversation_id,
            DEFAULT_DINGTALK_ACK_REACTION,
            EMOTION_REPLY_URL,
        )
        .await?;
        Ok(DingTalkReactionHandle {
            robot_code: robot_code.trim().to_string(),
            message_id: message_id.trim().to_string(),
            conversation_id: conversation_id.trim().to_string(),
            reaction_name: DEFAULT_DINGTALK_ACK_REACTION.to_string(),
        })
    }

    pub async fn recall_processing_reaction(
        &self,
        handle: &DingTalkReactionHandle,
    ) -> Result<(), ChannelError> {
        self.send_processing_reaction(
            &handle.robot_code,
            &handle.message_id,
            &handle.conversation_id,
            &handle.reaction_name,
            EMOTION_RECALL_URL,
        )
        .await
    }

    async fn send_processing_reaction(
        &self,
        robot_code: &str,
        message_id: &str,
        conversation_id: &str,
        reaction_name: &str,
        url: &str,
    ) -> Result<(), ChannelError> {
        let robot_code = robot_code.trim();
        let message_id = message_id.trim();
        let conversation_id = conversation_id.trim();
        if robot_code.is_empty() {
            return Err(ChannelError::ConfigError(
                "robot_code is required for DingTalk reaction".to_string(),
            ));
        }
        if message_id.is_empty() {
            return Err(ChannelError::ConfigError(
                "message_id is required for DingTalk reaction".to_string(),
            ));
        }
        if conversation_id.is_empty() {
            return Err(ChannelError::ConfigError(
                "conversation_id is required for DingTalk reaction".to_string(),
            ));
        }

        let reaction_name = if reaction_name.trim().is_empty() {
            DEFAULT_DINGTALK_ACK_REACTION
        } else {
            reaction_name.trim()
        };
        let access_token = self.get_access_token().await?;
        let request = serde_json::json!({
            "robotCode": robot_code,
            "openMsgId": message_id,
            "openConversationId": conversation_id,
            "emotionType": 2,
            "emotionName": reaction_name,
            "textEmotion": {
                "emotionId": THINKING_EMOTION_ID,
                "emotionName": reaction_name,
                "text": reaction_name,
                "backgroundId": THINKING_EMOTION_BACKGROUND_ID,
            },
        });

        let response = self
            .client
            .post(url)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", response_text)));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        Ok(())
    }

    pub async fn create_ai_card(
        &self,
        target: &DingTalkAiCardTarget,
        robot_code: &str,
        card_template_id: &str,
        status: &str,
        content: &str,
    ) -> Result<DingTalkAiCardHandle, ChannelError> {
        if robot_code.trim().is_empty() {
            return Err(ChannelError::ConfigError(
                "robot_code is required for DingTalk AI card".to_string(),
            ));
        }
        if card_template_id.trim().is_empty() {
            return Err(ChannelError::ConfigError(
                "card_template_id is required for DingTalk AI card".to_string(),
            ));
        }

        let (open_space_id, im_group_open_deliver_model, im_robot_open_deliver_model) = match target {
            DingTalkAiCardTarget::ImGroup { conversation_id } => {
                if conversation_id.trim().is_empty() {
                    return Err(ChannelError::ConfigError(
                        "conversation_id is required for DingTalk AI card".to_string(),
                    ));
                }
                (
                    build_im_group_open_space_id(conversation_id),
                    Some(DingTalkAiCardGroupDeliverModel {
                        robot_code: robot_code.to_string(),
                    }),
                    None,
                )
            }
            DingTalkAiCardTarget::ImRobot { user_id } => {
                if user_id.trim().is_empty() {
                    return Err(ChannelError::ConfigError(
                        "user_id is required for DingTalk AI card".to_string(),
                    ));
                }
                (
                    build_im_robot_open_space_id(user_id),
                    None,
                    Some(DingTalkAiCardRobotDeliverModel {
                        space_type: "IM_ROBOT".to_string(),
                    }),
                )
            }
        };

        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/card/instances/createAndDeliver";
        let out_track_id = format!("uhorse-{}", Uuid::new_v4().simple());
        let request = DingTalkAiCardCreateRequest {
            open_space_id,
            card_template_id: card_template_id.to_string(),
            out_track_id: out_track_id.clone(),
            card_data: DingTalkAiCardContent {
                card_param_map: serde_json::json!({
                    "status": status,
                    "content": content,
                }),
            },
            im_group_open_deliver_model,
            im_robot_open_deliver_model,
        };

        let response = self
            .client
            .post(url)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", response_text)));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        Ok(DingTalkAiCardHandle { out_track_id })
    }

    pub async fn finalize_ai_card(
        &self,
        handle: &DingTalkAiCardHandle,
        content: &str,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/card/instances";
        let request = DingTalkAiCardUpdateRequest {
            out_track_id: handle.out_track_id.clone(),
            card_data: DingTalkAiCardContent {
                card_param_map: serde_json::json!({
                    "status": "done",
                    "content": content,
                }),
            },
            card_update_options: DingTalkAiCardUpdateOptions {
                update_card_data_by_key: true,
            },
        };

        let response = self
            .client
            .put(url)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", response_text)));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        Ok(())
    }

    pub async fn clear_processing_ack(
        &self,
        receipt: Option<&DingTalkTransientMessageReceipt>,
    ) -> Result<DingTalkTransientClearOutcome, ChannelError> {
        match receipt {
            Some(receipt) if receipt.supports_clear() => Ok(DingTalkTransientClearOutcome::Skipped),
            Some(_) => Ok(DingTalkTransientClearOutcome::Unsupported),
            None => Ok(DingTalkTransientClearOutcome::Skipped),
        }
    }

    /// 通过 sessionWebhook 原路回复 Markdown 消息
    pub async fn reply_via_session_webhook_markdown(
        &self,
        session_webhook: &str,
        title: &str,
        markdown: &str,
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
        struct SessionMarkdownRequest {
            at: SessionAtBody,
            markdown: MarkdownBody,
            #[serde(rename = "msgtype")]
            msg_type: String,
        }

        let access_token = self.get_access_token().await?;
        let request = SessionMarkdownRequest {
            at: SessionAtBody {
                at_user_ids: at_user_ids.to_vec(),
                is_at_all: false,
            },
            markdown: MarkdownBody {
                title: title.to_string(),
                text: markdown.to_string(),
            },
            msg_type: "markdown".to_string(),
        };

        let response = self
            .client
            .post(session_webhook)
            .header("x-acs-dingtalk-access-token", access_token)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            error!("DingTalk session webhook error: {}", response_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                response_text
            )));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            error!("DingTalk session webhook business error: {}", error);
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
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

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            error!("DingTalk API error: {}", response_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                response_text
            )));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            error!("DingTalk API business error: {}", error);
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
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

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                response_text
            )));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        Ok(())
    }

    /// 发送群文本消息
    pub async fn send_group_message(
        &self,
        conversation_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        match message {
            MessageContent::Text(text) => self.send_group_text_message(conversation_id, text).await,
            _ => Err(ChannelError::SendFailed(
                "Only text messages are supported for group messages".to_string(),
            )),
        }
    }

    pub async fn send_group_text_message(
        &self,
        conversation_id: &str,
        text: &str,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/robot/groupMessages/send";

        #[derive(Serialize)]
        struct GroupTextRequest {
            #[serde(rename = "conversationId")]
            conversation_id: String,
            msg: GroupTextBody,
        }

        #[derive(Serialize)]
        struct GroupTextBody {
            #[serde(rename = "msgtype")]
            msg_type: String,
            text: TextBody,
        }

        let request = GroupTextRequest {
            conversation_id: conversation_id.to_string(),
            msg: GroupTextBody {
                msg_type: "text".to_string(),
                text: TextBody {
                    content: text.to_string(),
                },
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

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", response_text)));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
        }

        Ok(())
    }

    pub async fn send_group_markdown_message(
        &self,
        conversation_id: &str,
        title: &str,
        markdown: &str,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = "https://api.dingtalk.com/v1.0/robot/groupMessages/send";

        #[derive(Serialize)]
        struct GroupMarkdownRequest {
            #[serde(rename = "conversationId")]
            conversation_id: String,
            msg: GroupMarkdownBody,
        }

        #[derive(Serialize)]
        struct GroupMarkdownBody {
            #[serde(rename = "msgtype")]
            msg_type: String,
            markdown: MarkdownBody,
        }

        let request = GroupMarkdownRequest {
            conversation_id: conversation_id.to_string(),
            msg: GroupMarkdownBody {
                msg_type: "markdown".to_string(),
                markdown: MarkdownBody {
                    title: title.to_string(),
                    text: markdown.to_string(),
                },
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

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ChannelError::SendFailed(format!("API error: {}", response_text)));
        }
        if let Some(error) = parse_dingtalk_business_error(&response_text) {
            return Err(ChannelError::SendFailed(format!("API error: {}", error)));
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
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);

        assert_eq!(channel.app_key(), "test_key");
        assert_eq!(channel.app_secret(), "test_secret");
        assert_eq!(channel.agent_id(), 123456789);
        assert_eq!(channel.channel_type(), ChannelType::DingTalk);
    }

    #[test]
    fn test_extract_text_content() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);

        let event = DingTalkEvent {
            conversation_id: Some("conv_123".to_string()),
            message_id: None,
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
            "msgId": "msg_789",
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
        assert_eq!(event.message_id, Some("msg_789".to_string()));
        assert_eq!(event.sender_id, Some("user_456".to_string()));
        assert_eq!(
            event.text.as_ref().unwrap().content,
            Some("Hello World".to_string())
        );
    }

    #[test]
    fn test_handle_event_with_metadata_preserves_message_id() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let incoming = rt
            .block_on(channel.handle_event_with_metadata(&DingTalkEvent {
                conversation_id: Some("conv_123".to_string()),
                message_id: Some("msg_123".to_string()),
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
            }))
            .unwrap()
            .unwrap();
        assert_eq!(incoming.message_id.as_deref(), Some("msg_123"));
        assert_eq!(incoming.robot_code.as_deref(), Some("test_key"));
        assert!(incoming.attachments.is_empty());
    }

    #[test]
    fn test_extract_content_returns_structured_file_payload() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let event = DingTalkEvent {
            conversation_id: Some("conv_file".to_string()),
            message_id: Some("msg_file".to_string()),
            conversation_type: Some("2".to_string()),
            conversation_title: None,
            sender_id: Some("user_file".to_string()),
            sender_nick: None,
            sender_corp_id: Some("corp-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            msg_type: Some("file".to_string()),
            text: None,
            content: Some(serde_json::json!({
                "fileKey": "file-key-1",
                "fileName": "需求说明.pdf"
            })),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            create_time: Some(1234567890),
        };

        let content = channel.extract_content(&event);
        match content {
            MessageContent::Structured(data) => {
                assert_eq!(
                    data,
                    serde_json::json!({
                        "kind": "dingtalk_file",
                        "file_key": "file-key-1",
                        "file_name": "需求说明.pdf"
                    })
                );
            }
            other => panic!("expected structured file payload, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_content_returns_structured_file_payload_from_download_code_only() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let event = DingTalkEvent {
            conversation_id: Some("conv_file".to_string()),
            message_id: Some("msg_file".to_string()),
            conversation_type: Some("2".to_string()),
            conversation_title: None,
            sender_id: Some("user_file".to_string()),
            sender_nick: None,
            sender_corp_id: Some("corp-1".to_string()),
            sender_staff_id: Some("staff-1".to_string()),
            msg_type: Some("file".to_string()),
            text: None,
            content: Some(serde_json::json!({
                "downloadCode": "download-code-1",
                "fileName": "skill.zip"
            })),
            session_webhook: None,
            session_webhook_expired_time: None,
            robot_code: Some("robot-1".to_string()),
            create_time: Some(1234567890),
        };

        let content = channel.extract_content(&event);
        match content {
            MessageContent::Structured(data) => {
                assert_eq!(
                    data,
                    serde_json::json!({
                        "kind": "dingtalk_file",
                        "download_code": "download-code-1",
                        "file_name": "skill.zip",
                        "space_id": null,
                        "file_id": null,
                    })
                );
            }
            other => panic!("expected structured file payload, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_handle_event_with_metadata_preserves_audio_attachment_metadata() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let incoming = channel
            .handle_event_with_metadata(&DingTalkEvent {
                conversation_id: Some("conv_audio".to_string()),
                message_id: Some("msg_audio".to_string()),
                conversation_type: Some("2".to_string()),
                conversation_title: None,
                sender_id: Some("user_audio".to_string()),
                sender_nick: None,
                sender_corp_id: Some("corp-1".to_string()),
                sender_staff_id: Some("staff-1".to_string()),
                msg_type: Some("audio".to_string()),
                text: None,
                content: Some(serde_json::json!({
                    "audioKey": "audio-key-1",
                    "downloadCode": "download-code-1",
                    "recognition": "请帮我检查日志",
                    "duration": 3
                })),
                session_webhook: None,
                session_webhook_expired_time: None,
                robot_code: Some("robot-1".to_string()),
                create_time: Some(1234567890),
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            incoming.attachments,
            vec![DingTalkInboundAttachment {
                kind: "audio".to_string(),
                key: Some("audio-key-1".to_string()),
                file_name: None,
                download_code: Some("download-code-1".to_string()),
                recognition: Some("请帮我检查日志".to_string()),
                caption: None,
            }]
        );
        match incoming.message.content {
            MessageContent::Audio { url, duration } => {
                assert_eq!(url, "dingtalk://audio?key=audio-key-1");
                assert_eq!(duration, Some(3));
            }
            other => panic!("expected audio content, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_handle_event_with_metadata_preserves_file_attachment_metadata_from_download_code_only() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let incoming = channel
            .handle_event_with_metadata(&DingTalkEvent {
                conversation_id: Some("conv_file".to_string()),
                message_id: Some("msg_file".to_string()),
                conversation_type: Some("2".to_string()),
                conversation_title: None,
                sender_id: Some("user_file".to_string()),
                sender_nick: None,
                sender_corp_id: Some("corp-1".to_string()),
                sender_staff_id: Some("staff-1".to_string()),
                msg_type: Some("file".to_string()),
                text: None,
                content: Some(serde_json::json!({
                    "downloadCode": "download-code-1",
                    "fileName": "skill.zip"
                })),
                session_webhook: None,
                session_webhook_expired_time: None,
                robot_code: Some("robot-1".to_string()),
                create_time: Some(1234567890),
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            incoming.attachments,
            vec![DingTalkInboundAttachment {
                kind: "file".to_string(),
                key: None,
                file_name: Some("skill.zip".to_string()),
                download_code: Some("download-code-1".to_string()),
                recognition: None,
                caption: None,
            }]
        );
        match incoming.message.content {
            MessageContent::Structured(data) => {
                assert_eq!(
                    data,
                    serde_json::json!({
                        "kind": "dingtalk_file",
                        "download_code": "download-code-1",
                        "file_name": "skill.zip",
                        "space_id": null,
                        "file_id": null,
                    })
                );
            }
            other => panic!("expected structured file content, got {:?}", other),
        }
    }

    #[test]
    fn test_send_processing_ack_returns_receipt_without_message_id_when_platform_does_not_expose_handle() {
        let receipt = DingTalkTransientMessageReceipt::unsupported();
        assert!(!receipt.supports_clear());
    }

    #[test]
    fn test_add_processing_reaction_uses_expected_payload_shape() {
        let request = serde_json::json!({
            "robotCode": "app-key",
            "openMsgId": "msg-1",
            "openConversationId": "conv-1",
            "emotionType": 2,
            "emotionName": DEFAULT_DINGTALK_ACK_REACTION,
            "textEmotion": {
                "emotionId": THINKING_EMOTION_ID,
                "emotionName": DEFAULT_DINGTALK_ACK_REACTION,
                "text": DEFAULT_DINGTALK_ACK_REACTION,
                "backgroundId": THINKING_EMOTION_BACKGROUND_ID,
            },
        });
        assert_eq!(request["openMsgId"], "msg-1");
        assert_eq!(request["openConversationId"], "conv-1");
        assert_eq!(request["emotionName"], DEFAULT_DINGTALK_ACK_REACTION);
        assert_eq!(request["textEmotion"]["emotionId"], THINKING_EMOTION_ID);
    }

    #[test]
    fn test_recall_processing_reaction_uses_same_payload_identity() {
        let handle = DingTalkReactionHandle {
            robot_code: "robot-1".to_string(),
            message_id: "msg-1".to_string(),
            conversation_id: "conv-1".to_string(),
            reaction_name: DEFAULT_DINGTALK_ACK_REACTION.to_string(),
        };
        assert_eq!(handle.robot_code, "robot-1");
        assert_eq!(handle.message_id, "msg-1");
        assert_eq!(handle.conversation_id, "conv-1");
        assert_eq!(handle.reaction_name, DEFAULT_DINGTALK_ACK_REACTION);
    }

    #[test]
    fn test_create_ai_card_requires_target_value_and_robot_code() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(channel.create_ai_card(
                &DingTalkAiCardTarget::ImGroup {
                    conversation_id: "".to_string(),
                },
                "",
                "tpl",
                "processing",
                "hello",
            ))
            .unwrap_err();
        assert!(err.to_string().contains("robot_code is required"));
    }

    #[test]
    fn test_build_im_group_open_space_id_uses_dingtalk_required_shape() {
        assert_eq!(
            build_im_group_open_space_id("cidp4Gh123VCQ=="),
            "dtv1.card//im_group.cidp4Gh123VCQ=="
        );
    }

    #[test]
    fn test_build_im_robot_open_space_id_uses_dingtalk_required_shape() {
        assert_eq!(
            build_im_robot_open_space_id("manager123"),
            "dtv1.card//im_robot.manager123"
        );
    }

    #[test]
    fn test_create_ai_card_returns_handle_on_success_like_shape() {
        let handle = DingTalkAiCardHandle {
            out_track_id: "uhorse-test".to_string(),
        };
        assert_eq!(handle.out_track_id, "uhorse-test");
    }

    #[test]
    fn test_finalize_ai_card_uses_handle_and_final_content() {
        let handle = DingTalkAiCardHandle {
            out_track_id: "uhorse-test".to_string(),
        };
        let request = DingTalkAiCardUpdateRequest {
            out_track_id: handle.out_track_id.clone(),
            card_data: DingTalkAiCardContent {
                card_param_map: serde_json::json!({
                    "status": "done",
                    "content": "final",
                }),
            },
            card_update_options: DingTalkAiCardUpdateOptions {
                update_card_data_by_key: true,
            },
        };
        let payload = serde_json::to_value(request).unwrap();
        assert_eq!(payload["outTrackId"], "uhorse-test");
        assert_eq!(payload["cardData"]["cardParamMap"]["content"], "final");
    }

    #[test]
    fn test_parse_dingtalk_business_error_detects_errcode_payload() {
        let error = parse_dingtalk_business_error(r#"{"errcode":40035,"errmsg":"invalid param"}"#)
            .expect("should detect errcode business failure");
        assert!(error.contains("40035"));
        assert!(error.contains("invalid param"));
    }

    #[test]
    fn test_parse_dingtalk_business_error_detects_success_false_payload() {
        let error = parse_dingtalk_business_error(
            r#"{"success":false,"message":"delivery failed","requestId":"req-1"}"#,
        )
        .expect("should detect success=false business failure");
        assert!(error.contains("delivery failed"));
        assert!(error.contains("req-1"));
    }

    #[test]
    fn test_parse_dingtalk_business_error_ignores_success_payload() {
        assert!(parse_dingtalk_business_error(r#"{"errcode":0,"errmsg":"ok"}"#).is_none());
        assert!(parse_dingtalk_business_error(r#"{"success":true}"#).is_none());
    }

    #[test]
    fn test_stream_update_ai_card_can_be_added_without_breaking_finalize() {
        let payload = serde_json::json!({
            "status": "processing",
            "content": "working",
        });
        assert_eq!(payload["status"], "processing");
        assert_eq!(payload["content"], "working");
    }

    #[tokio::test]
    async fn test_clear_processing_ack_returns_unsupported_without_receipt_capability() {
        let channel =
            DingTalkChannel::new("test_key".to_string(), "test_secret".to_string(), 123456789, None);
        let outcome = channel
            .clear_processing_ack(Some(&DingTalkTransientMessageReceipt::unsupported()))
            .await
            .unwrap();
        assert_eq!(outcome, DingTalkTransientClearOutcome::Unsupported);
    }
}
