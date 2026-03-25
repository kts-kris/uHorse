//! # 飞书通道适配器
//!
//! 完整实现飞书机器人 API 集成，支持事件订阅。

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, MessageRole, Result, Session,
    UHorseError,
};

/// 飞书 API 响应
#[derive(Debug, Deserialize)]
struct FeishuResponse<T> {
    code: i64,
    msg: String,
    #[serde(default)]
    data: Option<T>,
}

/// 飞书访问令牌响应
#[derive(Debug, Deserialize, Default)]
struct AccessTokenData {
    #[serde(rename = "tenant_access_token")]
    tenant_access_token: String,
    expire: i64,
}

/// 飞书事件
#[derive(Debug, Deserialize)]
pub struct FeishuEvent {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub challenge: Option<String>,
    pub token: Option<String>,
    #[serde(rename = "ts")]
    pub timestamp: Option<String>,
    pub event: Option<EventData>,
    pub header: Option<EventHeader>,
}

/// 事件头
#[derive(Debug, Deserialize)]
pub struct EventHeader {
    pub event_id: Option<String>,
    pub event_type: Option<String>,
    pub create_time: Option<String>,
    pub token: Option<String>,
    #[serde(rename = "app_id")]
    pub app_id: Option<String>,
    #[serde(rename = "tenant_key")]
    pub tenant_key: Option<String>,
}

/// 事件数据
#[derive(Debug, Deserialize)]
pub struct EventData {
    pub sender: Option<Sender>,
    pub message: Option<MessageData>,
}

/// 发送者
#[derive(Debug, Deserialize)]
pub struct Sender {
    #[serde(rename = "sender_id")]
    pub sender_id: Option<SenderId>,
    #[serde(rename = "sender_type")]
    pub sender_type: Option<String>,
    #[serde(rename = "tenant_key")]
    pub tenant_key: Option<String>,
}

/// 发送者 ID
#[derive(Debug, Deserialize)]
pub struct SenderId {
    #[serde(rename = "union_id")]
    pub union_id: Option<String>,
    #[serde(rename = "user_id")]
    pub user_id: Option<String>,
    #[serde(rename = "open_id")]
    pub open_id: Option<String>,
}

/// 消息数据
#[derive(Debug, Deserialize)]
pub struct MessageData {
    pub message_id: Option<String>,
    #[serde(rename = "root_id")]
    pub root_id: Option<String>,
    #[serde(rename = "parent_id")]
    pub parent_id: Option<String>,
    #[serde(rename = "create_time")]
    pub create_time: Option<String>,
    #[serde(rename = "chat_id")]
    pub chat_id: Option<String>,
    #[serde(rename = "message_type")]
    pub message_type: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "mentions")]
    pub mentions: Option<Vec<Mention>>,
}

/// 提及
#[derive(Debug, Deserialize)]
pub struct Mention {
    pub key: Option<String>,
    pub id: Option<MentionId>,
    pub name: Option<String>,
    #[serde(rename = "tenant_key")]
    pub tenant_key: Option<String>,
}

/// 提及 ID
#[derive(Debug, Deserialize)]
pub struct MentionId {
    #[serde(rename = "union_id")]
    pub union_id: Option<String>,
    #[serde(rename = "user_id")]
    pub user_id: Option<String>,
    #[serde(rename = "open_id")]
    pub open_id: Option<String>,
}

/// 消息内容
#[derive(Debug, Deserialize)]
pub struct FeishuMessageContent {
    pub text: Option<String>,
    #[serde(rename = "file_key")]
    pub file_key: Option<String>,
    #[serde(rename = "image_key")]
    pub image_key: Option<String>,
    #[serde(rename = "audio_key")]
    pub audio_key: Option<String>,
    #[serde(rename = "media_id")]
    pub media_id: Option<String>,
    #[serde(rename = "duration")]
    pub duration: Option<i32>,
    pub title: Option<String>,
}

/// 发送消息请求
#[derive(Debug, Serialize)]
struct SendMessageRequest {
    #[serde(rename = "receive_id")]
    receive_id: String,
    msg_type: String,
    content: String,
}

/// 飞书通道
#[derive(Debug)]
pub struct FeishuChannel {
    app_id: String,
    app_secret: String,
    encrypt_key: Option<String>,
    verify_token: Option<String>,
    client: Client,
    running: Arc<RwLock<bool>>,
    tenant_access_token: Arc<RwLock<Option<String>>>,
    token_expires_at: Arc<RwLock<i64>>,
}

impl FeishuChannel {
    /// 创建新的飞书通道
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            encrypt_key: None,
            verify_token: None,
            client: Client::new(),
            running: Arc::new(RwLock::new(false)),
            tenant_access_token: Arc::new(RwLock::new(None)),
            token_expires_at: Arc::new(RwLock::new(0)),
        }
    }

    /// 设置加密密钥（用于事件验证）
    pub fn with_encrypt_key(mut self, encrypt_key: String) -> Self {
        self.encrypt_key = Some(encrypt_key);
        self
    }

    /// 设置验证令牌（用于事件验证）
    pub fn with_verify_token(mut self, verify_token: String) -> Self {
        self.verify_token = Some(verify_token);
        self
    }

    /// 获取 app id
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// 获取 app secret
    pub fn app_secret(&self) -> &str {
        &self.app_secret
    }

    /// 获取租户访问令牌（带缓存）
    pub async fn get_tenant_access_token(&self) -> Result<String, ChannelError> {
        let now = chrono::Utc::now().timestamp();

        // 检查缓存的令牌是否有效
        {
            let token = self.tenant_access_token.read().await;
            let expires_at = self.token_expires_at.read().await;
            if let Some(token) = token.as_ref() {
                if now < *expires_at - 300 {
                    // 提前 5 分钟刷新
                    return Ok(token.clone());
                }
            }
        }

        // 获取新令牌
        let url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";

        #[derive(Serialize)]
        struct TokenRequest {
            #[serde(rename = "app_id")]
            app_id: String,
            #[serde(rename = "app_secret")]
            app_secret: String,
        }

        let response = self
            .client
            .post(url)
            .json(&TokenRequest {
                app_id: self.app_id.clone(),
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

        let result: FeishuResponse<AccessTokenData> = response
            .json()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("JSON error: {}", e)))?;

        if result.code != 0 {
            return Err(ChannelError::ConfigError(format!(
                "API error: {} - {}",
                result.code, result.msg
            )));
        }

        let data = result
            .data
            .ok_or_else(|| ChannelError::InvalidResponse("No token data".to_string()))?;

        // 缓存令牌
        let expires_at = now + data.expire;
        *self.tenant_access_token.write().await = Some(data.tenant_access_token.clone());
        *self.token_expires_at.write().await = expires_at;

        info!(
            "Feishu tenant access token obtained, expires in {} seconds",
            data.expire
        );
        Ok(data.tenant_access_token)
    }

    /// 处理飞书事件回调 (原始 JSON)
    pub async fn handle_event_raw(
        &self,
        event_json: &str,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        debug!("Handling Feishu event (raw)");

        let event: FeishuEvent = serde_json::from_str(event_json).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to parse event JSON: {}", e))
        })?;

        self.handle_event(&event).await
    }

    /// 处理飞书事件
    pub async fn handle_event(
        &self,
        event: &FeishuEvent,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        // 处理 URL 验证挑战
        if let Some(challenge) = &event.challenge {
            debug!("Received URL verification challenge: {}", challenge);
            // 返回 None，调用方需要处理 challenge 响应
            return Ok(None);
        }

        // 验证 token
        if let Some(verify_token) = &self.verify_token {
            if let Some(token) = &event.token {
                if token != verify_token {
                    warn!("Invalid verify token in event");
                    return Err(ChannelError::ConfigError(
                        "Invalid verify token".to_string(),
                    ));
                }
            }
        }

        // 检查事件类型
        let event_type = event
            .event_type
            .as_ref()
            .or(event.header.as_ref().and_then(|h| h.event_type.as_ref()));

        match event_type {
            Some(t) if t.starts_with("im.message") => {
                // 处理消息事件
                let event_data = event
                    .event
                    .as_ref()
                    .ok_or_else(|| ChannelError::InvalidResponse("No event data".to_string()))?;

                let message_data = event_data
                    .message
                    .as_ref()
                    .ok_or_else(|| ChannelError::InvalidResponse("No message data".to_string()))?;

                let chat_id = message_data
                    .chat_id
                    .as_ref()
                    .ok_or_else(|| ChannelError::InvalidResponse("No chat_id".to_string()))?;

                // 创建会话
                let session = Session::new(ChannelType::Feishu, chat_id.clone());

                // 提取消息内容
                let message_content = self.extract_content(message_data);

                debug!(
                    "Processed Feishu message: chat_id={}, content={:?}",
                    chat_id, message_content
                );

                let timestamp = message_data
                    .create_time
                    .as_ref()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(0);

                let msg = Message::new(
                    session.id.clone(),
                    MessageRole::User,
                    message_content,
                    timestamp,
                );

                Ok(Some((session, msg)))
            }
            Some(t) => {
                debug!("Ignoring non-message event: {}", t);
                Ok(None)
            }
            None => {
                debug!("No event type in event");
                Ok(None)
            }
        }
    }

    /// 生成 challenge 响应
    pub fn challenge_response(&self, event: &FeishuEvent) -> Option<String> {
        event.challenge.as_ref().map(|challenge| {
            serde_json::json!({
                "challenge": challenge
            })
            .to_string()
        })
    }

    /// 提取消息内容
    fn extract_content(&self, message_data: &MessageData) -> MessageContent {
        let content = match &message_data.content {
            Some(c) => c,
            None => return MessageContent::Text("".to_string()),
        };

        // 解析内容 JSON
        let content_json: FeishuMessageContent = match serde_json::from_str(content) {
            Ok(c) => c,
            Err(_) => {
                // 如果解析失败，直接作为文本
                return MessageContent::Text(content.clone());
            }
        };

        let message_type = message_data.message_type.as_deref().unwrap_or("text");

        match message_type {
            "text" => {
                if let Some(text) = content_json.text {
                    return MessageContent::Text(text);
                }
            }
            "image" => {
                if let Some(image_key) = content_json.image_key {
                    return MessageContent::Image {
                        url: format!("feishu://image?key={}", image_key),
                        caption: None,
                    };
                }
            }
            "audio" => {
                if let Some(audio_key) = content_json.audio_key {
                    return MessageContent::Audio {
                        url: format!("feishu://audio?key={}", audio_key),
                        duration: content_json.duration.map(|d| d as u32),
                    };
                }
            }
            "media" => {
                if let Some(media_id) = content_json.media_id {
                    return MessageContent::Audio {
                        url: format!("feishu://media?id={}", media_id),
                        duration: content_json.duration.map(|d| d as u32),
                    };
                }
            }
            "file" => {
                if let Some(file_key) = content_json.file_key {
                    return MessageContent::Text(format!("[文件] {}", file_key));
                }
            }
            "post" => {
                // 富文本消息，尝试提取文本
                if let Some(text) = content_json.text {
                    return MessageContent::Text(text);
                }
                return MessageContent::Text("[富文本消息]".to_string());
            }
            "interactive" => {
                // 交互式卡片
                if let Some(text) = content_json.text {
                    return MessageContent::Text(text);
                }
                return MessageContent::Text("[交互式卡片]".to_string());
            }
            _ => {
                debug!("Unknown message type: {}", message_type);
            }
        }

        MessageContent::Text(content.clone())
    }

    /// 发送文本消息
    pub async fn send_text(&self, receive_id: &str, text: &str) -> Result<(), ChannelError> {
        let access_token = self.get_tenant_access_token().await?;
        let url = "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id";

        let request = SendMessageRequest {
            receive_id: receive_id.to_string(),
            msg_type: "text".to_string(),
            content: serde_json::json!({
                "text": text
            })
            .to_string(),
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Feishu API error: {}", error_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送富文本消息
    pub async fn send_rich_text(
        &self,
        receive_id: &str,
        title: &str,
        content: &str,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_tenant_access_token().await?;
        let url = "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id";

        let request = SendMessageRequest {
            receive_id: receive_id.to_string(),
            msg_type: "post".to_string(),
            content: serde_json::json!({
                "zh_cn": {
                    "title": title,
                    "content": [[{
                        "tag": "text",
                        "text": content
                    }]]
                }
            })
            .to_string(),
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
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

    /// 发送交互式卡片
    pub async fn send_card(
        &self,
        receive_id: &str,
        title: &str,
        elements: Vec<serde_json::Value>,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_tenant_access_token().await?;
        let url = "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id";

        let request = SendMessageRequest {
            receive_id: receive_id.to_string(),
            msg_type: "interactive".to_string(),
            content: serde_json::json!({
                "config": {
                    "wide_screen_mode": true
                },
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": title
                    }
                },
                "elements": elements
            })
            .to_string(),
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
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

    /// 回复消息
    pub async fn reply_message(
        &self,
        message_id: &str,
        content: &MessageContent,
    ) -> Result<(), ChannelError> {
        let access_token = self.get_tenant_access_token().await?;
        let url = format!(
            "https://open.feishu.cn/open-apis/im/v1/messages/{}/reply?receive_id_type=chat_id",
            message_id
        );

        let (msg_type, content_json) = match content {
            MessageContent::Text(text) => ("text", serde_json::json!({ "text": text })),
            MessageContent::Image { url, caption: _ } => (
                "image",
                serde_json::json!({
                    "image_key": url.replace("feishu://image?key=", "")
                }),
            ),
            _ => {
                return Err(ChannelError::SendFailed(
                    "Unsupported message type for reply".to_string(),
                ));
            }
        };

        #[derive(Serialize)]
        struct ReplyRequest {
            msg_type: String,
            content: String,
        }

        let request = ReplyRequest {
            msg_type: msg_type.to_string(),
            content: content_json.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
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
}

#[async_trait]
impl Channel for FeishuChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    #[instrument(skip(self, message))]
    async fn send_message(
        &self,
        user_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        debug!("Sending Feishu message to {}: {:?}", user_id, message);

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
                self.send_rich_text(user_id, "数据", &format!("```\n{}\n```", json))
                    .await?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_webhook(
        &self,
        payload: &[u8],
        signature: Option<&str>,
    ) -> Result<bool, ChannelError> {
        debug!("Verifying Feishu webhook");

        // 飞书签名验证
        if let Some(_encrypt_key) = &self.encrypt_key {
            if let Some(sig) = signature {
                // TODO: 实现签名验证
                // 使用 encrypt_key + timestamp + payload 计算 HMAC-SHA256
                debug!("Signature provided: {}", sig);
            }
        }

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting Feishu channel");

        // 测试获取访问令牌
        match self.get_tenant_access_token().await {
            Ok(token) => {
                info!("Feishu channel connected, token obtained");
                debug!("Access token: {}...", &token[..20.min(token.len())]);
            }
            Err(e) => {
                error!("Failed to connect to Feishu: {}", e);
                return Err(UHorseError::ChannelError(e));
            }
        }

        *self.running.write().await = true;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Feishu channel");
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
    fn test_feishu_channel_creation() {
        let channel = FeishuChannel::new("test_app_id".to_string(), "test_app_secret".to_string());

        assert_eq!(channel.app_id(), "test_app_id");
        assert_eq!(channel.app_secret(), "test_app_secret");
        assert_eq!(channel.channel_type(), ChannelType::Feishu);
    }

    #[test]
    fn test_challenge_response() {
        let channel = FeishuChannel::new("test_app_id".to_string(), "test_app_secret".to_string());

        let event = FeishuEvent {
            event_type: None,
            challenge: Some("test_challenge_123".to_string()),
            token: None,
            timestamp: None,
            event: None,
            header: None,
        };

        let response = channel.challenge_response(&event);
        assert!(response.is_some());
        assert!(response.unwrap().contains("test_challenge_123"));
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{
            "type": "im.message.receive_v1",
            "event": {
                "sender": {
                    "sender_id": {
                        "user_id": "user_123"
                    }
                },
                "message": {
                    "message_id": "msg_456",
                    "chat_id": "chat_789",
                    "message_type": "text",
                    "content": "{\"text\":\"Hello\"}",
                    "create_time": "1234567890"
                }
            }
        }"#;

        let event: FeishuEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, Some("im.message.receive_v1".to_string()));
        assert!(event.event.is_some());
    }

    #[test]
    fn test_extract_text_content() {
        let channel = FeishuChannel::new("test_app_id".to_string(), "test_app_secret".to_string());

        let message_data = MessageData {
            message_id: Some("msg_123".to_string()),
            root_id: None,
            parent_id: None,
            create_time: Some("1234567890".to_string()),
            chat_id: Some("chat_456".to_string()),
            message_type: Some("text".to_string()),
            content: Some(r#"{"text":"Hello World"}"#.to_string()),
            mentions: None,
        };

        let content = channel.extract_content(&message_data);
        assert!(matches!(content, MessageContent::Text(t) if t == "Hello World"));
    }
}
