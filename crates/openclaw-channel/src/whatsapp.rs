//! # WhatsApp 通道适配器
//!
//! 实现 WhatsApp Business API 集成。

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

/// WhatsApp 通道
#[derive(Debug, Clone)]
pub struct WhatsAppChannel {
    access_token: String,
    phone_number_id: String,
    api_version: String,
    client: Client,
    running: Arc<RwLock<bool>>,
}

impl WhatsAppChannel {
    /// 创建新的 WhatsApp 通道
    pub fn new(access_token: String, phone_number_id: String) -> Self {
        Self {
            access_token,
            phone_number_id,
            api_version: "v18.0".to_string(),
            client: Client::new(),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 设置 API 版本
    pub fn with_api_version(mut self, version: String) -> Self {
        self.api_version = version;
        self
    }

    /// 获取 API 基础 URL
    fn api_url(&self, path: &str) -> String {
        format!("https://graph.facebook.com/{}/{}", self.api_version, path)
    }

    /// 验证 webhook
    pub fn verify_webhook_token(&self, mode: &str, token: &str, _challenge: &str) -> Result<bool> {
        // TODO: 从配置中读取验证 token
        // WhatsApp webhook 验证
        debug!("WhatsApp webhook verification: mode={}, token={}", mode, token);

        if mode == "subscribe" {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 发送文本消息
    async fn send_text(&self, to: &str, text: &str) -> Result<(), ChannelError> {
        let url = self.api_url(&format!("{}/messages", self.phone_number_id));

        #[derive(Serialize)]
        struct SendMessage<'a> {
            messaging_product: &'a str,
            to: &'a str,
            r#type: &'a str,
            text: TextContent<'a>,
        }

        #[derive(Serialize)]
        struct TextContent<'a> {
            preview_url: bool,
            body: &'a str,
        }

        let message = SendMessage {
            messaging_product: "whatsapp",
            to,
            r#type: "text",
            text: TextContent {
                preview_url: false,
                body: text,
            },
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&message)
            .send()
            .await
                                    .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("WhatsApp API error: {}", error_text)));
        }

        Ok(())
    }

    /// 发送媒体消息
    async fn send_media(&self, to: &str, media_type: &str, url: &str) -> Result<(), ChannelError> {
        let api_url = self.api_url(&format!("{}/messages", self.phone_number_id));

        #[derive(Serialize)]
        struct SendMessage<'a> {
            messaging_product: &'a str,
            to: &'a str,
            r#type: &'a str,
            media: MediaContent<'a>,
        }

        #[derive(Serialize)]
        struct MediaContent<'a> {
            media_type: &'a str,
            url: &'a str,
        }

        let message = SendMessage {
            messaging_product: "whatsapp",
            to,
            r#type: media_type,
            media: MediaContent {
                media_type,
                url,
            },
        };

        let response = self.client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&message)
            .send()
            .await
                                    .map_err(|e| ChannelError::SendFailed(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("WhatsApp API error: {}", error_text)));
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WhatsApp
    }

    #[instrument(skip(self, message))]
    async fn send_message(&self, user_id: &str, message: &MessageContent) -> Result<(), ChannelError> {
        debug!("Sending WhatsApp message to {}: {:?}", user_id, message);

        // 确保手机号格式正确
        let to = if !user_id.starts_with('+') {
            &format!("+{}", user_id)
        } else {
            user_id
        };

        match message {
            MessageContent::Text(text) => {
                self.send_text(to, text).await?;
            }
            MessageContent::Image { url, .. } => {
                self.send_media(to, "image", url).await?;
            }
            MessageContent::Audio { url, .. } => {
                self.send_media(to, "audio", url).await?;
            }
            MessageContent::Structured(data) => {
                if let Some(text) = data.as_str() {
                    self.send_text(to, text).await?;
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn verify_webhook(&self, _payload: &[u8], _signature: Option<&str>) -> Result<bool, ChannelError> {
        // WhatsApp 使用 HMAC-SHA256 签名
        // 这里简化处理
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> Result<()> {
        info!("Starting WhatsApp channel (phone_number_id: {})", self.phone_number_id);

        // 测试 API 连接
        let url = self.api_url(&format!("{}/?fields=name", self.phone_number_id));
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| OpenClawError::ChannelError(ChannelError::ConfigError(format!("Failed to connect: {}", e))))?;

        if response.status().is_success() {
            info!("WhatsApp API connection successful");
        } else {
            warn!("WhatsApp API connection test returned non-success status");
        }

        *self.running.write().await = true;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping WhatsApp channel");
        *self.running.write().await = false;
        Ok(())
    }

    fn is_running(&self) -> bool {
        *self.running.blocking_read()
    }
}

// ============== WhatsApp 类型 ==============

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookEntry {
    pub messaging_product: String,
    pub display_phone_number: Option<String>,
    pub metadata: Option<WhatsAppMetadata>,
    pub contacts: Option<Vec<WhatsAppContact>>,
    pub messages: Option<Vec<WhatsAppMessage>>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMetadata {
    pub display_phone_number: String,
    pub phone_number_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppContact {
    pub profile: Option<WhatsAppProfile>,
    pub wa_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppProfile {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMessage {
    pub from: String,
    pub id: String,
    pub timestamp: String,
    pub r#type: String,
    pub text: Option<WhatsAppText>,
    pub audio: Option<WhatsAppAudio>,
    pub image: Option<WhatsAppImage>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppText {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppAudio {
    pub file: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppImage {
    pub file: String,
    pub caption: Option<String>,
    pub mime_type: Option<String>,
}

