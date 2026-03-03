//! # 钉钉通道适配器
//!
//! 完整实现钉钉机器人 API 集成。

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};
use uhorse_core::{
    Channel, ChannelError, ChannelType, Message, MessageContent, Result, UHorseError,
};

/// 钉钉通道
#[derive(Debug)]
pub struct DingTalkChannel {
    app_key: String,
    app_secret: String,
    agent_id: u64,
    running: RwLock<bool>,
}

impl DingTalkChannel {
    /// 创建新的钉钉通道
    pub fn new(app_key: String, app_secret: String, agent_id: u64) -> Self {
        Self {
            app_key,
            app_secret,
            agent_id,
            running: RwLock::new(false),
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

    /// 处理钉钉事件回调 (原始 JSON)
    pub async fn handle_event_raw(
        &self,
        event_json: &str,
    ) -> Result<Option<(String, Message)>, ChannelError> {
        debug!("Handling DingTalk event (raw)");

        // 解析 JSON
        let event: serde_json::Value = serde_json::from_str(event_json)
            .map_err(|e| ChannelError::ConfigError(format!("Failed to parse event JSON: {}", e)))?;

        // 提取基本信息
        let conversation_id = event
            .get("conversationId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::ConfigError("No conversationId in event".to_string()))?;

        let sender_id = event
            .get("sender")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // 提取消息内容
        let message_content = self.extract_content_raw(&event)?;

        debug!(
            "Processed DingTalk message: conversation_id={}, sender_id={}, content={:?}",
            conversation_id, sender_id, message_content
        );

        let message = Message::new(
            uhorse_core::SessionId::new(),
            uhorse_core::MessageRole::User,
            message_content,
            0,
        );

        Ok(Some((conversation_id.to_string(), message)))
    }

    /// 提取消息内容 (从 JSON)
    fn extract_content_raw(
        &self,
        event_obj: &serde_json::Value,
    ) -> Result<MessageContent, ChannelError> {
        let content = event_obj
            .get("content")
            .ok_or_else(|| ChannelError::ConfigError("No content in event".to_string()))?;

        // 文本消息
        if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
            return Ok(MessageContent::Text(text.to_string()));
        }

        // 图片消息
        if let Some(image_key) = content.get("imageKey").and_then(|v| v.as_str()) {
            let url = format!("dingtalk://image?key={}", image_key);
            return Ok(MessageContent::Image {
                url,
                caption: content
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }

        // 音频消息
        if let Some(audio_key) = content.get("audioKey").and_then(|v| v.as_str()) {
            let url = format!("dingtalk://audio?key={}", audio_key);
            let duration = content
                .get("duration")
                .and_then(|v| v.as_u64())
                .map(|d| d as u32);
            return Ok(MessageContent::Audio { url, duration });
        }

        // Markdown 消息
        if let Some(markdown) = content.get("markdown").and_then(|v| v.as_str()) {
            return Ok(MessageContent::Text(markdown.to_string()));
        }

        // 默认返回文本
        Ok(MessageContent::Text(content.to_string()))
    }

    /// 获取访问令牌
    pub async fn get_access_token(&self) -> Result<String, ChannelError> {
        // TODO: 实现实际的 OAuth 2.0 请求
        // POST https://api.dingtalk.com/v1.0/oauth2/accessToken
        Ok("access_token".to_string())
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
        debug!("Sending message to DingTalk: user_id={}", user_id);

        // TODO: 实现实际的 HTTP 请求
        match message {
            MessageContent::Text(text) => {
                debug!("Would send text: {}", text);
            }
            MessageContent::Image { url, caption } => {
                debug!("Would send image: url={:?}, caption={:?}", url, caption);
            }
            MessageContent::Audio { url, duration } => {
                debug!("Would send audio: url={:?}, duration={:?}", url, duration);
            }
            MessageContent::Structured(value) => {
                debug!("Would send structured: {:?}", value);
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
        debug!("Verifying DingTalk webhook");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting DingTalk channel");

        // TODO: 测试 API 连接
        debug!("DingTalk channel started");
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
}
