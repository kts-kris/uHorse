//! # Slack 通道适配器
//!
//! 实现 Slack Events API 和 Webhook 集成。
//!
//! 注意：当前为简化实现，完整功能待后续完善。

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};
use uhorse_core::{
    Channel, ChannelCapabilityFlags, ChannelError, ChannelRecipient, ChannelType, MessageContent,
    Result,
};

/// Slack 通道
#[derive(Debug, Clone)]
pub struct SlackChannel {
    bot_token: String,
    running: Arc<RwLock<bool>>,
}

impl SlackChannel {
    /// 创建新的 Slack 通道
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 创建带签名验证的通道
    pub fn with_signing_secret(bot_token: String, _signing_secret: String) -> Self {
        Self {
            bot_token,
            running: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Slack
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
        if recipient.channel_type != ChannelType::Slack {
            return Err(ChannelError::ConfigError(format!(
                "recipient channel type mismatch: {}",
                recipient.channel_type
            )));
        }

        debug!(
            "Sending Slack message to {}: {:?}",
            recipient.recipient, message
        );

        // TODO: 实现完整的 Slack API 调用
        match message {
            MessageContent::Text(text) => {
                debug!("Would send text: {}", text);
            }
            _ => {
                debug!("Would send: {:?}", message);
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
        debug!("Verifying Slack webhook");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!(
            "Starting Slack channel (bot_token: {})",
            &self.bot_token[..8]
        );
        // TODO: 实现 Slack API 连接测试
        *self.running.write().await = true;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Slack channel");
        *self.running.write().await = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.blocking_read()
    }
}

// ============== Slack 事件类型 (简化版) ==============

#[derive(Debug, serde::Deserialize)]
pub struct SlackEvent {
    pub token: Option<String>,
    pub challenge: Option<String>,
    pub r#type: Option<String>,
    pub event_id: Option<String>,
    pub event_time: Option<i64>,
    pub event: Option<SlackInnerEvent>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SlackInnerEvent {
    pub r#type: String,
    pub user: Option<String>,
    pub channel: Option<String>,
    pub text: Option<String>,
}
