//! # 钉钉通道适配器
//!
//! 完整实现钉钉机器人 API 集成，支持 Stream 模式。

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, MessageRole, Result, Session,
    SessionId, UHorseError,
};

/// 钉钉 API 响应
#[derive(Debug, Deserialize)]
struct DingTalkResponse<T> {
    errcode: i64,
    errmsg: String,
    #[serde(default)]
    result: Option<T>,
}

/// 钉钉访问令牌响应
#[derive(Debug, Deserialize)]
struct AccessTokenResult {
    access_token: String,
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
    #[serde(rename = "msgtype")]
    pub msg_type: Option<String>,
    pub text: Option<TextContent>,
    pub content: Option<serde_json::Value>,
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

/// 钉钉通道
#[derive(Debug)]
pub struct DingTalkChannel {
    app_key: String,
    app_secret: String,
    agent_id: u64,
    client: Client,
    running: Arc<RwLock<bool>>,
    access_token: Arc<RwLock<Option<String>>>,
    token_expires_at: Arc<RwLock<i64>>,
}

impl DingTalkChannel {
    /// 创建新的钉钉通道
    pub fn new(app_key: String, app_secret: String, agent_id: u64) -> Self {
        Self {
            app_key,
            app_secret,
            agent_id,
            client: Client::new(),
            running: Arc::new(RwLock::new(false)),
            access_token: Arc::new(RwLock::new(None)),
            token_expires_at: Arc::new(RwLock::new(0)),
        }
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

        // 检查缓存的令牌是否有效
        {
            let token = self.access_token.read().await;
            let expires_at = self.token_expires_at.read().await;
            if let Some(token) = token.as_ref() {
                if now < *expires_at - 300 {
                    // 提前 5 分钟刷新
                    return Ok(token.clone());
                }
            }
        }

        // 获取新令牌
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

        // 缓存令牌
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

        self.handle_event(&event).await
    }

    /// 处理钉钉事件
    pub async fn handle_event(
        &self,
        event: &DingTalkEvent,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        let conversation_id = match &event.conversation_id {
            Some(id) => id,
            None => {
                debug!("No conversation_id in event");
                return Ok(None);
            }
        };

        // 创建会话
        let session = Session::new(ChannelType::DingTalk, conversation_id.clone());

        // 提取消息内容
        let message_content = self.extract_content(event);

        debug!(
            "Processed DingTalk message: conversation_id={}, sender_id={}, content={:?}",
            conversation_id,
            event.sender_id.as_deref().unwrap_or("unknown"),
            message_content
        );

        let msg = Message::new(
            session.id.clone(),
            MessageRole::User,
            message_content,
            event.create_time.unwrap_or(0) as u64,
        );

        Ok(Some((session, msg)))
    }

    /// 处理 Stream 消息
    pub async fn handle_stream_message(
        &self,
        stream_msg: &StreamMessage,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        debug!(
            "Handling DingTalk stream message: topic={}",
            stream_msg.topic
        );

        // 解析消息数据
        let event: DingTalkEvent = serde_json::from_str(&stream_msg.data).map_err(|e| {
            ChannelError::InvalidResponse(format!("Failed to parse stream data: {}", e))
        })?;

        self.handle_event(&event).await
    }

    /// 提取消息内容
    fn extract_content(&self, event: &DingTalkEvent) -> MessageContent {
        // 文本消息
        if let Some(text) = &event.text {
            if let Some(content) = &text.content {
                return MessageContent::Text(content.clone());
            }
        }

        // 从 content 字段提取
        if let Some(content) = &event.content {
            // 文本类型
            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                return MessageContent::Text(text.to_string());
            }

            // 图片类型
            if let Some(image_key) = content.get("imageKey").and_then(|v| v.as_str()) {
                return MessageContent::Image {
                    url: format!("dingtalk://image?key={}", image_key),
                    caption: content
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                };
            }

            // 音频类型
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

            // 文件类型
            if let Some(file_key) = content.get("fileKey").and_then(|v| v.as_str()) {
                let file_name = content
                    .get("fileName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("文件");
                return MessageContent::Text(format!("[{}] {}", file_name, file_key));
            }

            // Markdown 类型
            if let Some(markdown) = content.get("markdown").and_then(|v| v.as_str()) {
                return MessageContent::Text(markdown.to_string());
            }
        }

        MessageContent::Text("[不支持的消息类型]".to_string())
    }

    /// 发送文本消息
    pub async fn send_text(&self, user_id: &str, text: &str) -> Result<(), ChannelError> {
        let access_token = self.get_access_token().await?;
        let url = format!(
            "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend?access_token={}",
            access_token
        );

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
            .post(&url)
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
        let url = format!(
            "https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend?access_token={}",
            access_token
        );

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
            .post(&url)
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
        let url = format!(
            "https://api.dingtalk.com/v1.0/robot/groupMessages/send?access_token={}",
            access_token
        );

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
                    .post(&url)
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
                // 钉钉图片需要先上传获取 media_id
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
        payload: &[u8],
        signature: Option<&str>,
    ) -> Result<bool, ChannelError> {
        debug!("Verifying DingTalk webhook");

        // 钉钉签名验证
        if let Some(sig) = signature {
            // TODO: 实现签名验证
            // 使用 app_secret + timestamp + payload 计算 HMAC-SHA256
            debug!("Signature provided: {}", sig);
        }

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting DingTalk channel");

        // 测试获取访问令牌
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
            msg_type: Some("text".to_string()),
            text: Some(TextContent {
                content: Some("Hello".to_string()),
            }),
            content: None,
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
