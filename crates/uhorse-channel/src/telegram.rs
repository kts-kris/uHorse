//! # Telegram 通道适配器
//!
//! 完整实现 Telegram Bot API 集成。

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};
use uhorse_core::{
    Channel, ChannelCapabilityFlags, ChannelError, ChannelRecipient, ChannelType, Message,
    MessageContent, MessageRole, Result, Session, UHorseError,
};

/// Telegram API 响应
#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

/// Telegram Bot 信息
#[derive(Debug, Clone, Deserialize)]
struct BotUser {
    first_name: String,
    username: Option<String>,
}

/// Telegram 更新
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
    pub edited_message: Option<TelegramMessage>,
    pub callback_query: Option<CallbackQuery>,
}

/// Telegram 消息
#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub date: i64,
    pub text: Option<String>,
    pub caption: Option<String>,
    pub photo: Option<Vec<PhotoSize>>,
    pub audio: Option<Audio>,
    pub document: Option<Document>,
    pub voice: Option<Voice>,
}

/// Telegram 用户
#[derive(Debug, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub language_code: Option<String>,
}

/// Telegram 聊天
#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

/// 图片尺寸
#[derive(Debug, Deserialize)]
pub struct PhotoSize {
    pub file_id: String,
    pub file_unique_id: String,
    pub width: i32,
    pub height: i32,
    pub file_size: Option<i64>,
}

/// 音频
#[derive(Debug, Deserialize)]
pub struct Audio {
    pub file_id: String,
    pub file_unique_id: String,
    pub duration: i32,
    pub performer: Option<String>,
    pub title: Option<String>,
    pub file_size: Option<i64>,
}

/// 文档
#[derive(Debug, Deserialize)]
pub struct Document {
    pub file_id: String,
    pub file_unique_id: String,
    pub thumb: Option<PhotoSize>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

/// 语音
#[derive(Debug, Deserialize)]
pub struct Voice {
    pub file_id: String,
    pub file_unique_id: String,
    pub duration: i32,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

/// 回调查询
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: TelegramUser,
    pub message: Option<TelegramMessage>,
    pub inline_message_id: Option<String>,
    pub chat_instance: String,
    pub data: Option<String>,
}

/// 发送消息请求
#[derive(Debug, Serialize)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
}

/// 发送图片请求
#[derive(Debug, Serialize)]
struct SendPhotoRequest {
    chat_id: i64,
    photo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
}

/// Telegram 通道
#[derive(Debug, Clone)]
pub struct TelegramChannel {
    bot_token: String,
    client: Client,
    api_base: String,
    running: Arc<RwLock<bool>>,
    bot_info: Arc<RwLock<Option<BotUser>>>,
}

impl TelegramChannel {
    /// 创建新的 Telegram 通道
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            client: Client::new(),
            api_base: "https://api.telegram.org".to_string(),
            running: Arc::new(RwLock::new(false)),
            bot_info: Arc::new(RwLock::new(None)),
        }
    }

    /// 使用自定义 API 基础 URL（用于测试）
    pub fn with_api_base(mut self, api_base: String) -> Self {
        self.api_base = api_base;
        self
    }

    /// 获取 bot token
    pub fn bot_token(&self) -> &str {
        &self.bot_token
    }

    /// 构建 API URL
    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.api_base, self.bot_token, method)
    }

    /// 获取 Bot 信息
    async fn get_me(&self) -> Result<BotUser, ChannelError> {
        let url = self.api_url("getMe");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;

        let result: TelegramResponse<BotUser> = response
            .json()
            .await
            .map_err(|e| ChannelError::InvalidResponse(format!("JSON error: {}", e)))?;

        if result.ok {
            result
                .result
                .ok_or_else(|| ChannelError::InvalidResponse("No result".to_string()))
        } else {
            Err(ChannelError::SendFailed(
                result
                    .description
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    /// 处理 Telegram 更新 (原始 JSON)
    pub async fn handle_update_raw(
        &self,
        update_json: &str,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        debug!("Handling Telegram update (raw)");

        let update: Update = serde_json::from_str(update_json)
            .map_err(|e| ChannelError::InvalidResponse(format!("JSON error: {}", e)))?;

        self.handle_update(&update).await
    }

    /// 处理 Telegram 更新
    pub async fn handle_update(
        &self,
        update: &Update,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        // 获取消息（优先原始消息，其次编辑过的消息）
        let message = update.message.as_ref().or(update.edited_message.as_ref());

        // 处理回调查询
        if let Some(callback) = &update.callback_query {
            return self.handle_callback(callback).await;
        }

        let message = match message {
            Some(m) => m,
            None => {
                debug!("No message in update");
                return Ok(None);
            }
        };

        // 创建会话
        let chat_id = message.chat.id.to_string();
        let session = Session::new(ChannelType::Telegram, chat_id.clone());

        // 提取消息内容
        let message_content = self.extract_content(message);

        debug!(
            "Processed Telegram message: chat_id={}, content={:?}",
            chat_id, message_content
        );

        let msg = Message::new(
            session.id.clone(),
            MessageRole::User,
            message_content,
            update.update_id as u64,
        );

        Ok(Some((session, msg)))
    }

    /// 处理回调查询
    async fn handle_callback(
        &self,
        callback: &CallbackQuery,
    ) -> Result<Option<(Session, Message)>, ChannelError> {
        let chat_id = callback
            .message
            .as_ref()
            .map(|m| m.chat.id.to_string())
            .unwrap_or_else(|| callback.from.id.to_string());

        let session = Session::new(ChannelType::Telegram, chat_id.clone());

        let content = callback
            .data
            .as_ref()
            .map(|d| MessageContent::Text(format!("[callback] {}", d)))
            .unwrap_or_else(|| MessageContent::Text("[callback]".to_string()));

        let msg = Message::new(session.id.clone(), MessageRole::User, content, 0);

        Ok(Some((session, msg)))
    }

    /// 提取消息内容
    fn extract_content(&self, message: &TelegramMessage) -> MessageContent {
        // 文本消息
        if let Some(text) = &message.text {
            return MessageContent::Text(text.clone());
        }

        // 图片消息
        if let Some(photos) = &message.photo {
            if let Some(largest) = photos.last() {
                return MessageContent::Image {
                    url: largest.file_id.clone(),
                    caption: message.caption.clone(),
                };
            }
        }

        // 音频消息
        if let Some(audio) = &message.audio {
            return MessageContent::Audio {
                url: audio.file_id.clone(),
                duration: Some(audio.duration as u32),
            };
        }

        // 语音消息
        if let Some(voice) = &message.voice {
            return MessageContent::Audio {
                url: voice.file_id.clone(),
                duration: Some(voice.duration as u32),
            };
        }

        // 文档消息
        if let Some(doc) = &message.document {
            let text = match &doc.file_name {
                Some(name) => format!("[文件] {}", name),
                None => "[文件]".to_string(),
            };
            return MessageContent::Text(text);
        }

        MessageContent::Text("[不支持的消息类型]".to_string())
    }

    /// 发送文本消息
    pub async fn send_text(&self, chat_id: i64, text: &str) -> Result<(), ChannelError> {
        self.send_text_with_options(chat_id, text, None, None).await
    }

    /// 发送文本消息（带选项）
    pub async fn send_text_with_options(
        &self,
        chat_id: i64,
        text: &str,
        parse_mode: Option<&str>,
        reply_to: Option<i64>,
    ) -> Result<(), ChannelError> {
        let url = self.api_url("sendMessage");

        let request = SendMessageRequest {
            chat_id,
            text: text.to_string(),
            parse_mode: parse_mode.map(|s| s.to_string()),
            reply_to_message_id: reply_to,
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
            error!("Telegram API error: {}", error_text);
            return Err(ChannelError::SendFailed(format!(
                "API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送图片
    pub async fn send_photo(
        &self,
        chat_id: i64,
        photo: &str,
        caption: Option<&str>,
    ) -> Result<(), ChannelError> {
        let url = self.api_url("sendPhoto");

        let request = SendPhotoRequest {
            chat_id,
            photo: photo.to_string(),
            caption: caption.map(|s| s.to_string()),
            parse_mode: None,
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

    /// 设置 Webhook
    pub async fn set_webhook(&self, webhook_url: &str) -> Result<(), ChannelError> {
        let url = self.api_url("setWebhook");

        #[derive(Serialize)]
        struct SetWebhookRequest {
            url: String,
        }

        let response = self
            .client
            .post(&url)
            .json(&SetWebhookRequest {
                url: webhook_url.to_string(),
            })
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::ConfigError(format!(
                "Webhook setup failed: {}",
                error_text
            )));
        }

        info!("Telegram webhook set to: {}", webhook_url);
        Ok(())
    }

    /// 删除 Webhook
    pub async fn delete_webhook(&self) -> Result<(), ChannelError> {
        let url = self.api_url("deleteWebhook");

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::ConfigError(format!(
                "Delete webhook failed: {}",
                error_text
            )));
        }

        info!("Telegram webhook deleted");
        Ok(())
    }

    /// 回答回调查询
    pub async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
        show_alert: bool,
    ) -> Result<(), ChannelError> {
        let url = self.api_url("answerCallbackQuery");

        #[derive(Serialize)]
        struct AnswerCallbackRequest {
            callback_query_id: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            text: Option<String>,
            show_alert: bool,
        }

        let response = self
            .client
            .post(&url)
            .json(&AnswerCallbackRequest {
                callback_query_id: callback_query_id.to_string(),
                text: text.map(|s| s.to_string()),
                show_alert,
            })
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
impl Channel for TelegramChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn capability_flags(&self) -> ChannelCapabilityFlags {
        ChannelCapabilityFlags::SEND_TO_RECIPIENT
    }

    #[instrument(skip(self, message))]
    async fn send_to_recipient(
        &self,
        recipient: &ChannelRecipient,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        if recipient.channel_type != ChannelType::Telegram {
            return Err(ChannelError::ConfigError(format!(
                "recipient channel type mismatch: {}",
                recipient.channel_type
            )));
        }

        debug!(
            "Sending Telegram message to {}: {:?}",
            recipient.recipient, message
        );

        let chat_id: i64 = recipient
            .recipient
            .parse()
            .map_err(|_| ChannelError::ConfigError("Invalid chat_id".to_string()))?;

        match message {
            MessageContent::Text(text) => {
                self.send_text(chat_id, text).await?;
            }
            MessageContent::Image { url, caption } => {
                self.send_photo(chat_id, url, caption.as_deref()).await?;
            }
            MessageContent::Audio { url, duration } => {
                // Telegram 使用 voice 或 audio 端点
                let text = format!("[音频] {} ({}秒)", url, duration.unwrap_or(0));
                self.send_text(chat_id, &text).await?;
            }
            MessageContent::Structured(data) => {
                let json = serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| "Invalid JSON".to_string());
                self.send_text(chat_id, &format!("```\n{}\n```", json))
                    .await?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_webhook(
        &self,
        _payload: &[u8],
        _signature: Option<&str>,
    ) -> Result<bool, ChannelError> {
        // Telegram 不使用签名验证，而是使用 secret token
        debug!("Verifying Telegram webhook");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting Telegram channel");

        // 测试 API 连接
        match self.get_me().await {
            Ok(bot_info) => {
                info!(
                    "Telegram Bot connected: @{} ({})",
                    bot_info.username.as_deref().unwrap_or("unknown"),
                    bot_info.first_name
                );
                *self.bot_info.write().await = Some(bot_info);
            }
            Err(e) => {
                error!("Failed to connect to Telegram API: {}", e);
                return Err(UHorseError::ChannelError(e));
            }
        }

        *self.running.write().await = true;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Telegram channel");
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
    fn test_telegram_channel_creation() {
        let channel = TelegramChannel::new("test_token".to_string());
        assert_eq!(channel.bot_token(), "test_token");
        assert_eq!(channel.channel_type(), ChannelType::Telegram);
    }

    #[test]
    fn test_extract_text_content() {
        let channel = TelegramChannel::new("test_token".to_string());

        let message = TelegramMessage {
            message_id: 1,
            from: None,
            chat: TelegramChat {
                id: 123,
                chat_type: "private".to_string(),
                title: None,
                username: None,
                first_name: None,
                last_name: None,
            },
            date: 0,
            text: Some("Hello".to_string()),
            caption: None,
            photo: None,
            audio: None,
            document: None,
            voice: None,
        };

        let content = channel.extract_content(&message);
        assert!(matches!(content, MessageContent::Text(t) if t == "Hello"));
    }

    #[test]
    fn test_extract_photo_content() {
        let channel = TelegramChannel::new("test_token".to_string());

        let message = TelegramMessage {
            message_id: 1,
            from: None,
            chat: TelegramChat {
                id: 123,
                chat_type: "private".to_string(),
                title: None,
                username: None,
                first_name: None,
                last_name: None,
            },
            date: 0,
            text: None,
            caption: Some("Photo caption".to_string()),
            photo: Some(vec![PhotoSize {
                file_id: "photo_id".to_string(),
                file_unique_id: "unique_id".to_string(),
                width: 100,
                height: 100,
                file_size: None,
            }]),
            audio: None,
            document: None,
            voice: None,
        };

        let content = channel.extract_content(&message);
        assert!(
            matches!(content, MessageContent::Image { url, caption } if url == "photo_id" && caption == Some("Photo caption".to_string()))
        );
    }

    #[test]
    fn test_update_deserialization() {
        let json = r#"{
            "update_id": 12345,
            "message": {
                "message_id": 1,
                "from": {
                    "id": 123,
                    "is_bot": false,
                    "first_name": "Test",
                    "username": "testuser"
                },
                "chat": {
                    "id": 123,
                    "type": "private",
                    "first_name": "Test"
                },
                "date": 1234567890,
                "text": "Hello"
            }
        }"#;

        let update: Update = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 12345);
        assert!(update.message.is_some());
        assert_eq!(
            update.message.as_ref().unwrap().text,
            Some("Hello".to_string())
        );
    }
}
