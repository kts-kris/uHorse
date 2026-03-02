//! # Discord 通道适配器
//!
//! 实现 Discord Bot API 集成。

use openclaw_core::{
    Channel, MessageContent, ChannelError, OpenClawError,
    ChannelType, Message, MessageRole, Session, SessionId,
    Result,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Discord 通道
#[derive(Debug, Clone)]
pub struct DiscordChannel {
    bot_token: String,
    client: Client,
    application_id: Option<String>,
    running: Arc<RwLock<bool>>,
}

impl DiscordChannel {
    /// 创建新的 Discord 通道
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            client: Client::new(),
            application_id: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 获取当前用户信息
    pub async fn get_current_user(&self) -> Result<DiscordUser, ChannelError> {
        let response = self.client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
            .map_err(|e| ChannelError::ConfigError(format!("Failed to get user: {}", e)))?;

        if !response.status().is_success() {
            return Err(ChannelError::ConfigError("Discord API error".to_string()));
        }

        response
            .json()
            .await
            .map_err(|e| ChannelError::ConfigError(format!("Failed to parse user: {}", e)))
    }

    /// 发送消息到 Discord
    async fn send_to_discord(&self, channel_id: &str, content: &str) -> Result<(), ChannelError> {
        let url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);

        #[derive(Serialize)]
        struct SendMessage<'a> {
            content: &'a str,
        }

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&SendMessage { content })
            .send()
            .await
                        .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("Discord API error: {}", error_text)));
        }

        Ok(())
    }

    /// 发送嵌入消息
    async fn send_embed(&self, channel_id: &str, embed: &DiscordEmbed) -> Result<(), ChannelError> {
        let url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);

        #[derive(Serialize)]
        struct SendMessage<'a> {
            embed: &'a DiscordEmbed,
        }

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&SendMessage { embed })
            .send()
            .await
                        .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("Discord API error: {}", error_text)));
        }

        Ok(())
    }

    /// 创建 DM 通道
    pub async fn create_dm(&self, user_id: &str) -> Result<String, ChannelError> {
        let url = "https://discord.com/api/v10/users/@me/channels";

        #[derive(Serialize)]
        struct CreateDm {
            recipient_id: String,
        }

        let response = self.client
            .post(url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&CreateDm {
                recipient_id: user_id.to_string(),
            })
            .send()
            .await
                        .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(ChannelError::SendFailed("Failed to create DM".to_string()));
        }

        let dm_channel: DiscordChannelResponse = response
            .json()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to parse DM: {}", e)))?;

        Ok(dm_channel.id)
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    #[instrument(skip(self, message))]
    async fn send_message(&self, user_id: &str, message: &MessageContent) -> Result<(), ChannelError> {
        debug!("Sending Discord message to {}: {:?}", user_id, message);

        // Discord 需要先创建 DM 通道
        let dm_channel = self.create_dm(user_id).await?;

        match message {
            MessageContent::Text(text) => {
                self.send_to_discord(&dm_channel, text).await?;
            }
            _ => {
                let text = format!("{:?}", message);
                self.send_to_discord(&dm_channel, &text).await?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_webhook(&self, _payload: &[u8], signature: Option<&str>) -> Result<bool, ChannelError> {
        if signature.is_some() {
            debug!("Discord webhook signature present");
        }
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting Discord channel");

        let user = self.get_current_user().await
            .map_err(|e| OpenClawError::ChannelError(e))?;
        info!("Connected as Discord bot: {}", user.username);

        *self.running.write().await = true;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Discord channel");
        *self.running.write().await = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.blocking_read()
    }
}

// ============== Discord 类型 ==============

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub bot: Option<bool>,
    pub system: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordChannelResponse {
    pub id: String,
    pub r#type: i32,
    pub last_message_id: Option<String>,
    pub recipient_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub fields: Option<Vec<EmbedField>>,
}

#[derive(Debug, Serialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: Option<bool>,
}

// Discord Gateway 事件
#[derive(Debug, Deserialize)]
pub struct DiscordGatewayEvent {
    pub op: i64,
    pub d: serde_json::Value,
    pub s: Option<i64>,
    pub t: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordMessageCreate {
    pub id: String,
    pub channel_id: String,
    pub author: DiscordUser,
    pub content: Option<String>,
    pub attachments: Vec<DiscordAttachment>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordAttachment {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub content_type: Option<String>,
    pub size: u64,
}

