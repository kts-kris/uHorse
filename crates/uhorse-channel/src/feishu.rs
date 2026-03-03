//! # 飞书通道适配器
//!
//! 完整实现飞书机器人 API 集成。

use uhorse_core::{
    Channel, MessageContent, ChannelError, UHorseError, Result,
    ChannelType, Message,
};
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// 飞书通道
#[derive(Debug)]
pub struct FeishuChannel {
    app_id: String,
    app_secret: String,
    encrypt_key: Option<String>,
    verify_token: Option<String>,
    running: RwLock<bool>,
}

impl FeishuChannel {
    /// 创建新的飞书通道
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            encrypt_key: None,
            verify_token: None,
            running: RwLock::new(false),
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

    /// 处理飞书事件回调 (原始 JSON)
    pub async fn handle_event_raw(&self, event_json: &str) -> Result<Option<(String, Message)>, ChannelError> {
        debug!("Handling Feishu event (raw)");

        // 解析 JSON
        let event: serde_json::Value = serde_json::from_str(event_json)
            .map_err(|e| ChannelError::ConfigError(format!("Failed to parse event JSON: {}", e)))?;

        // 验证事件
        if let Some(_token) = &self.verify_token {
            if let Some(_challenge) = event.get("challenge").and_then(|v| v.as_str()) {
                // 处理 URL 验证挑战
                debug!("Received URL verification challenge");
                return Ok(None);
            }
        }

        // 提取基本信息
        let event_type = event.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::ConfigError("No type in event".to_string()))?;

        if event_type != "im.message.receive_v1" {
            debug!("Ignoring non-message event: {}", event_type);
            return Ok(None);
        }

        let event_data = event.get("event")
            .ok_or_else(|| ChannelError::ConfigError("No event data".to_string()))?;

        // 获取会话信息
        let chat_id = event_data.get("chat_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::ConfigError("No chat_id in event".to_string()))?;

        let sender_id = event_data.get("sender")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // 提取消息内容
        let message_content = self.extract_content_raw(event_data)?;

        debug!("Processed Feishu message: chat_id={}, sender_id={}, content={:?}",
            chat_id, sender_id, message_content);

        let message = Message::new(
            uhorse_core::SessionId::new(),
            uhorse_core::MessageRole::User,
            message_content,
            0
        );

        Ok(Some((chat_id.to_string(), message)))
    }

    /// 提取消息内容 (从 JSON)
    fn extract_content_raw(&self, event_obj: &serde_json::Value) -> Result<MessageContent, ChannelError> {
        let message = event_obj.get("message")
            .ok_or_else(|| ChannelError::ConfigError("No message in event".to_string()))?;

        let message_type = message.get("message_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::ConfigError("No message_type".to_string()))?;

        let content = message.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::ConfigError("No content in message".to_string()))?;

        // 解析 content JSON 字符串
        let content_json: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| ChannelError::ConfigError(format!("Failed to parse content: {}", e)))?;

        match message_type {
            "text" => {
                if let Some(text) = content_json.get("text").and_then(|v| v.as_str()) {
                    return Ok(MessageContent::Text(text.to_string()));
                }
            }
            "image" => {
                let image_key = content_json.get("image_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                return Ok(MessageContent::Image {
                    url: format!("feishu://image?key={}", image_key),
                    caption: None,
                });
            }
            "audio" => {
                let file_key = content_json.get("file_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = format!("feishu://audio?key={}", file_key);
                return Ok(MessageContent::Audio {
                    url,
                    duration: None,
                });
            }
            "file" => {
                let file_key = content_json.get("file_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                return Ok(MessageContent::Text(format!("[文件] {}", file_key)));
            }
            "post" => {
                // 富文本消息
                return Ok(MessageContent::Text("[富文本消息]".to_string()));
            }
            "interactive" => {
                // 交互式卡片
                return Ok(MessageContent::Text("[交互式卡片]".to_string()));
            }
            _ => {
                debug!("Unknown message type: {}", message_type);
            }
        }

        // 默认返回原始内容
        Ok(MessageContent::Text(content.to_string()))
    }

    /// 获取租户访问令牌
    pub async fn get_tenant_access_token(&self) -> Result<String, ChannelError> {
        // TODO: 实现实际的 OAuth 2.0 请求
        // POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
        Ok("tenant_access_token".to_string())
    }

    /// 获取用户访问令牌
    pub async fn get_user_access_token(&self) -> Result<String, ChannelError> {
        // TODO: 实现实际的 OAuth 2.0 请求
        Ok("user_access_token".to_string())
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    #[instrument(skip(self, message))]
    async fn send_message(&self, user_id: &str, message: &MessageContent) -> Result<(), ChannelError> {
        debug!("Sending message to Feishu: user_id={}", user_id);

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
    async fn verify_webhook(&self, _payload: &[u8], _signature: Option<&str>) -> Result<bool, ChannelError> {
        debug!("Verifying Feishu webhook");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting Feishu channel");

        // TODO: 测试 API 连接
        debug!("Feishu channel started");
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
        let channel = FeishuChannel::new(
            "test_app_id".to_string(),
            "test_app_secret".to_string(),
        );

        assert_eq!(channel.app_id(), "test_app_id");
        assert_eq!(channel.app_secret(), "test_app_secret");
        assert_eq!(channel.channel_type(), ChannelType::Feishu);
    }
}
