//! # Telegram 通道适配器
//!
//! 完整实现 Telegram Bot API 集成。

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, MessageRole, Result, Session,
    SessionId, UHorseError,
};

/// Telegram 通道
#[derive(Debug, Clone)]
pub struct TelegramChannel {
    bot_token: String,
    running: Arc<RwLock<bool>>,
}

impl TelegramChannel {
    /// 创建新的 Telegram 通道
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 获取 bot token
    pub fn bot_token(&self) -> &str {
        &self.bot_token
    }

    /// 处理 Telegram 更新 (原始 JSON)
    pub async fn handle_update_raw(&self, update_json: &str) -> Result<Option<(Session, Message)>> {
        debug!("Handling Telegram update (raw)");

        // 解析 JSON
        let update: serde_json::Value = serde_json::from_str(update_json)?;

        // 提取基本信息
        let message_obj = update
            .get("message")
            .or_else(|| update.get("edited_message"))
            .ok_or_else(|| UHorseError::InternalError("No message in update".to_string()))?;

        let chat = message_obj
            .get("chat")
            .ok_or_else(|| UHorseError::InternalError("No chat in message".to_string()))?;

        // 获取 chat_id，支持数字 ID 和字符串 username
        let chat_id = if let Some(id) = chat.get("id").and_then(|v| v.as_i64()) {
            // 数字 ID（私聊）
            id.to_string()
        } else if let Some(username) = chat.get("username").and_then(|v| v.as_str()) {
            // 字符串 username（公开群组或频道）
            format!("@{}", username)
        } else {
            return Err(UHorseError::InternalError(
                "No valid chat identifier".to_string(),
            ));
        };

        // 创建会话
        let session = Session::new(ChannelType::Telegram, chat_id.clone());

        // 提取消息内容
        let message_content = self.extract_content_raw(message_obj)?;

        debug!(
            "Processed Telegram message: chat_id={}, content={:?}",
            chat_id, message_content
        );

        let message = Message::new(session.id.clone(), MessageRole::User, message_content, 0);

        Ok(Some((session, message)))
    }

    /// 提取消息内容 (从 JSON)
    fn extract_content_raw(&self, message_obj: &serde_json::Value) -> Result<MessageContent> {
        // 文本消息
        if let Some(text) = message_obj.get("text").and_then(|v| v.as_str()) {
            return Ok(MessageContent::Text(text.to_string()));
        }

        // 图片消息
        if let Some(photo_array) = message_obj.get("photo").and_then(|v| v.as_array()) {
            if let Some(largest) = photo_array.last() {
                let url = largest
                    .get("file_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let caption = message_obj
                    .get("caption")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return Ok(MessageContent::Image { url, caption });
            }
        }

        // 音频消息
        if let Some(audio) = message_obj.get("audio") {
            let url = audio
                .get("file_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let duration = audio
                .get("duration")
                .and_then(|v| v.as_i64())
                .map(|d| d as u32);
            return Ok(MessageContent::Audio { url, duration });
        }

        // 默认返回文本
        Ok(MessageContent::Text("Unsupported message type".to_string()))
    }

    /// 发送消息到 Telegram API
    async fn send_to_api(
        &self,
        chat_id: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        let client = reqwest::Client::new();
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);

        #[derive(serde::Serialize)]
        struct RequestBody<'a> {
            chat_id: &'a str,
            #[serde(flatten)]
            payload: &'a serde_json::Value,
        }

        let response = client
            .post(&url)
            .json(&RequestBody { chat_id, payload })
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "Telegram API error: {}",
                error_text
            )));
        }

        Ok(())
    }

    /// 发送文本消息
    async fn send_text(&self, chat_id: &str, text: &str) -> Result<(), ChannelError> {
        let payload = serde_json::json!({
            "text": text
        });
        self.send_to_api(chat_id, &payload).await
    }

    /// 发送图片
    async fn send_photo(
        &self,
        chat_id: &str,
        url: &str,
        caption: Option<&str>,
    ) -> Result<(), ChannelError> {
        let mut payload = serde_json::json!({
            "photo": url
        });

        if let Some(caption) = caption {
            payload["caption"] = serde_json::Value::String(caption.to_string());
        }

        self.send_to_api(chat_id, &payload).await
    }

    /// 发送音频
    async fn send_audio(
        &self,
        chat_id: &str,
        url: &str,
        duration: Option<u32>,
    ) -> Result<(), ChannelError> {
        let mut payload = serde_json::json!({
            "audio": url
        });

        if let Some(d) = duration {
            payload["duration"] = serde_json::Value::Number(d.into());
        }

        self.send_to_api(chat_id, &payload).await
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    #[instrument(skip(self, message))]
    async fn send_message(
        &self,
        user_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError> {
        debug!("Sending Telegram message to {}: {:?}", user_id, message);

        match message {
            MessageContent::Text(text) => {
                self.send_text(user_id, text).await?;
            }
            MessageContent::Image { url, caption } => {
                self.send_photo(user_id, url, caption.as_deref()).await?;
            }
            MessageContent::Audio { url, duration } => {
                self.send_audio(user_id, url, *duration).await?;
            }
            MessageContent::Structured(data) => {
                let json =
                    serde_json::to_string(data).unwrap_or_else(|_| "Invalid JSON".to_string());
                self.send_text(user_id, &json).await?;
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
        debug!("Verifying Telegram webhook");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting Telegram channel");

        // 测试 API 连接
        let client = reqwest::Client::new();
        let url = format!("https://api.telegram.org/bot{}/getMe", self.bot_token);

        let response = client.get(&url).send().await.map_err(|e| {
            UHorseError::ChannelError(ChannelError::ConfigError(format!(
                "Failed to connect: {}",
                e
            )))
        })?;

        if response.status().is_success() {
            info!("Telegram API connection successful");
        } else {
            return Err(UHorseError::ChannelError(ChannelError::ConfigError(
                "Telegram API auth failed".to_string(),
            )));
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
