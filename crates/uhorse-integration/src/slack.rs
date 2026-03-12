//! Slack Integration
//!
//! Slack 消息推送集成

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Slack 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Bot User OAuth Token (xoxb-...)
    pub bot_token: String,
    /// 默认频道
    #[serde(default)]
    pub default_channel: Option<String>,
    /// 应用 ID
    #[serde(default)]
    pub app_id: Option<String>,
    /// 客户端 ID
    #[serde(default)]
    pub client_id: Option<String>,
}

impl SlackConfig {
    /// 创建新配置
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            default_channel: None,
            app_id: None,
            client_id: None,
        }
    }

    /// 设置默认频道
    pub fn with_default_channel(mut self, channel: impl Into<String>) -> Self {
        self.default_channel = Some(channel.into());
        self
    }
}

/// Slack 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    /// 消息时间戳
    pub ts: String,
    /// 频道
    pub channel: String,
    /// 消息文本
    #[serde(default)]
    pub text: Option<String>,
    /// 用户
    #[serde(default)]
    pub user: Option<String>,
    /// 附件
    #[serde(default)]
    pub attachments: Vec<SlackAttachment>,
    /// Blocks
    #[serde(default)]
    pub blocks: Vec<serde_json::Value>,
}

/// Slack 附件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachment {
    /// 标题
    #[serde(default)]
    pub title: Option<String>,
    /// 标题链接
    #[serde(default)]
    pub title_link: Option<String>,
    /// 文本
    #[serde(default)]
    pub text: Option<String>,
    /// 颜色 (good, warning, danger 或 hex)
    #[serde(default)]
    pub color: Option<String>,
    /// 作者名称
    #[serde(default)]
    pub author_name: Option<String>,
    /// 作者链接
    #[serde(default)]
    pub author_link: Option<String>,
    /// 作者图标
    #[serde(default)]
    pub author_icon: Option<String>,
    /// 字段
    #[serde(default)]
    pub fields: Vec<SlackField>,
    /// Footer
    #[serde(default)]
    pub footer: Option<String>,
    /// Footer 图标
    #[serde(default)]
    pub footer_icon: Option<String>,
    /// 时间戳
    #[serde(default)]
    pub ts: Option<i64>,
    /// 图片 URL
    #[serde(default)]
    pub image_url: Option<String>,
    /// 缩略图 URL
    #[serde(default)]
    pub thumb_url: Option<String>,
}

impl SlackAttachment {
    /// 创建新附件
    pub fn new() -> Self {
        Self {
            title: None,
            title_link: None,
            text: None,
            color: None,
            author_name: None,
            author_link: None,
            author_icon: None,
            fields: Vec::new(),
            footer: None,
            footer_icon: None,
            ts: None,
            image_url: None,
            thumb_url: None,
        }
    }

    /// 设置标题
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置文本
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// 设置颜色
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 添加字段
    pub fn add_field(mut self, title: impl Into<String>, value: impl Into<String>, short: bool) -> Self {
        self.fields.push(SlackField {
            title: title.into(),
            value: value.into(),
            short,
        });
        self
    }
}

impl Default for SlackAttachment {
    fn default() -> Self {
        Self::new()
    }
}

/// Slack 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackField {
    /// 字段标题
    pub title: String,
    /// 字段值
    pub value: String,
    /// 是否短字段
    #[serde(default)]
    pub short: bool,
}

/// 发送消息请求
#[derive(Debug, Clone, Serialize)]
struct PostMessageRequest {
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<SlackAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_broadcast: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unfurl_links: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unfurl_media: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_emoji: Option<String>,
}

/// 发送消息响应
#[derive(Debug, Clone, Deserialize)]
struct PostMessageResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    ts: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

/// 用户信息
#[derive(Debug, Clone, Deserialize)]
struct UserInfoResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    user: Option<SlackUser>,
}

/// Slack 用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackUser {
    /// 用户 ID
    pub id: String,
    /// 用户名
    pub name: String,
    /// 显示名称
    #[serde(default)]
    pub real_name: Option<String>,
    /// 显示名称 (profile)
    #[serde(default)]
    pub display_name: Option<String>,
    /// 邮箱
    #[serde(default)]
    pub email: Option<String>,
    /// 头像
    #[serde(default)]
    pub profile_image_url: Option<String>,
    /// 是否为机器人
    #[serde(default)]
    pub is_bot: Option<bool>,
}

/// 频道信息响应
#[derive(Debug, Clone, Deserialize)]
struct ChannelInfoResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    channel: Option<SlackChannel>,
}

/// Slack 频道
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannel {
    /// 频道 ID
    pub id: String,
    /// 频道名称
    pub name: String,
    /// 是否为频道
    #[serde(default)]
    pub is_channel: Option<bool>,
    /// 是否为群组
    #[serde(default)]
    pub is_group: Option<bool>,
    /// 是否为私聊
    #[serde(default)]
    pub is_im: Option<bool>,
    /// 成员数量
    #[serde(default)]
    pub num_members: Option<i32>,
    /// 主题
    #[serde(default)]
    pub topic: Option<ChannelTopic>,
    /// 用途
    #[serde(default)]
    pub purpose: Option<ChannelPurpose>,
}

/// 频道主题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTopic {
    /// 主题内容
    pub value: String,
    /// 创建者
    #[serde(default)]
    pub creator: Option<String>,
    /// 更新时间
    #[serde(default)]
    pub last_set: Option<i64>,
}

/// 频道用途
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPurpose {
    /// 用途内容
    pub value: String,
    /// 创建者
    #[serde(default)]
    pub creator: Option<String>,
    /// 更新时间
    #[serde(default)]
    pub last_set: Option<i64>,
}

/// Slack 客户端
pub struct SlackClient {
    /// 配置
    config: SlackConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// API 基础 URL
    api_url: String,
}

impl SlackClient {
    /// 创建新客户端
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            api_url: "https://slack.com/api".to_string(),
        }
    }

    /// 发送简单文本消息
    pub async fn send_message(
        &self,
        channel: &str,
        text: &str,
    ) -> crate::Result<SlackMessage> {
        self.send_message_with_options(channel, text, None, None, None).await
    }

    /// 发送带选项的消息
    pub async fn send_message_with_options(
        &self,
        channel: &str,
        text: &str,
        attachments: Option<Vec<SlackAttachment>>,
        blocks: Option<Vec<serde_json::Value>>,
        thread_ts: Option<&str>,
    ) -> crate::Result<SlackMessage> {
        let url = format!("{}/chat.postMessage", self.api_url);

        let request = PostMessageRequest {
            channel: channel.to_string(),
            text: Some(text.to_string()),
            attachments,
            blocks,
            thread_ts: thread_ts.map(|s| s.to_string()),
            reply_broadcast: None,
            unfurl_links: None,
            unfurl_media: None,
            username: None,
            icon_url: None,
            icon_emoji: None,
        };

        info!("Sending Slack message to channel: {}", channel);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::SlackError(format!(
                "Failed to send message: {}",
                error_text
            )));
        }

        let result: PostMessageResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("Parse error: {}", e)))?;

        if !result.ok {
            return Err(crate::IntegrationError::SlackError(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        Ok(SlackMessage {
            ts: result.ts.unwrap_or_default(),
            channel: result.channel.unwrap_or_else(|| channel.to_string()),
            text: Some(text.to_string()),
            user: None,
            attachments: vec![],
            blocks: vec![],
        })
    }

    /// 发送带附件的消息
    pub async fn send_attachment(
        &self,
        channel: &str,
        text: &str,
        attachments: Vec<SlackAttachment>,
    ) -> crate::Result<SlackMessage> {
        self.send_message_with_options(channel, text, Some(attachments), None, None)
            .await
    }

    /// 发送带 Blocks 的消息
    pub async fn send_blocks(
        &self,
        channel: &str,
        text: &str,
        blocks: Vec<serde_json::Value>,
    ) -> crate::Result<SlackMessage> {
        self.send_message_with_options(channel, text, None, Some(blocks), None)
            .await
    }

    /// 回复消息
    pub async fn reply_to_message(
        &self,
        channel: &str,
        thread_ts: &str,
        text: &str,
    ) -> crate::Result<SlackMessage> {
        self.send_message_with_options(channel, text, None, None, Some(thread_ts))
            .await
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        channel: &str,
        ts: &str,
        text: &str,
    ) -> crate::Result<SlackMessage> {
        let url = format!("{}/chat.update", self.api_url);

        let body = serde_json::json!({
            "channel": channel,
            "ts": ts,
            "text": text
        });

        info!("Updating Slack message {} in channel {}", ts, channel);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::SlackError(format!(
                "Failed to update message: {}",
                error_text
            )));
        }

        let result: PostMessageResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("Parse error: {}", e)))?;

        if !result.ok {
            return Err(crate::IntegrationError::SlackError(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        Ok(SlackMessage {
            ts: result.ts.unwrap_or_else(|| ts.to_string()),
            channel: result.channel.unwrap_or_else(|| channel.to_string()),
            text: Some(text.to_string()),
            user: None,
            attachments: vec![],
            blocks: vec![],
        })
    }

    /// 删除消息
    pub async fn delete_message(&self, channel: &str, ts: &str) -> crate::Result<()> {
        let url = format!("{}/chat.delete", self.api_url);

        let body = serde_json::json!({
            "channel": channel,
            "ts": ts
        });

        info!("Deleting Slack message {} from channel {}", ts, channel);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::SlackError(format!(
                "Failed to delete message: {}",
                error_text
            )));
        }

        let result: PostMessageResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("Parse error: {}", e)))?;

        if !result.ok {
            return Err(crate::IntegrationError::SlackError(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        Ok(())
    }

    /// 获取用户信息
    pub async fn get_user(&self, user_id: &str) -> crate::Result<SlackUser> {
        let url = format!("{}/users.info?user={}", self.api_url, user_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::IntegrationError::SlackError(format!(
                "User not found: {}",
                user_id
            )));
        }

        let result: UserInfoResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("Parse error: {}", e)))?;

        if !result.ok {
            return Err(crate::IntegrationError::SlackError(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        result.user.ok_or_else(|| {
            crate::IntegrationError::SlackError("User not found in response".to_string())
        })
    }

    /// 获取频道信息
    pub async fn get_channel(&self, channel_id: &str) -> crate::Result<SlackChannel> {
        let url = format!("{}/conversations.info?channel={}", self.api_url, channel_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.bot_token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::IntegrationError::SlackError(format!(
                "Channel not found: {}",
                channel_id
            )));
        }

        let result: ChannelInfoResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::SlackError(format!("Parse error: {}", e)))?;

        if !result.ok {
            return Err(crate::IntegrationError::SlackError(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        result.channel.ok_or_else(|| {
            crate::IntegrationError::SlackError("Channel not found in response".to_string())
        })
    }

    /// 发送告警消息 (便捷方法)
    pub async fn send_alert(
        &self,
        channel: &str,
        title: &str,
        message: &str,
        severity: AlertSeverity,
    ) -> crate::Result<SlackMessage> {
        let color = match severity {
            AlertSeverity::Critical => "danger",
            AlertSeverity::Warning => "warning",
            AlertSeverity::Info => "good",
        };

        let attachment = SlackAttachment::new()
            .with_title(title)
            .with_text(message)
            .with_color(color)
            .add_field("Severity", severity.to_string(), true)
            .add_field("Timestamp", Utc::now().to_rfc3339(), true);

        self.send_attachment(channel, "", vec![attachment]).await
    }
}

/// 告警严重程度
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// 严重
    Critical,
    /// 警告
    Warning,
    /// 信息
    Info,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Critical => write!(f, "Critical"),
            AlertSeverity::Warning => write!(f, "Warning"),
            AlertSeverity::Info => write!(f, "Info"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_config() {
        let config = SlackConfig::new("xoxb-test-token")
            .with_default_channel("#general");

        assert_eq!(config.bot_token, "xoxb-test-token");
        assert_eq!(config.default_channel, Some("#general".to_string()));
    }

    #[test]
    fn test_attachment_builder() {
        let attachment = SlackAttachment::new()
            .with_title("Test Title")
            .with_text("Test Text")
            .with_color("danger")
            .add_field("Field 1", "Value 1", true)
            .add_field("Field 2", "Value 2", false);

        assert_eq!(attachment.title, Some("Test Title".to_string()));
        assert_eq!(attachment.text, Some("Test Text".to_string()));
        assert_eq!(attachment.color, Some("danger".to_string()));
        assert_eq!(attachment.fields.len(), 2);
    }

    #[test]
    fn test_alert_severity_display() {
        assert_eq!(AlertSeverity::Critical.to_string(), "Critical");
        assert_eq!(AlertSeverity::Warning.to_string(), "Warning");
        assert_eq!(AlertSeverity::Info.to_string(), "Info");
    }
}
