//! # Session Key - 会话键管理
//!
//! OpenClaw 风格的 Session Key 生成和管理。
//!
//! ## Session Key 格式
//!
//! ```text
//! {channel_type}:{channel_user_id}[:{team_id}]
//! ```
//!
//! ## 示例
//!
//! - `telegram:user123` - Telegram 用户会话
//! - `slack:user456:T123` - Slack 工作区会话
//! - `discord:user789` - Discord 用户会话
//!
//! ## 用途
//!
//! - **会话隔离**: 相同用户在不同 channel 有独立会话
//! - **多租户支持**: team_id 实现工作区隔离
//! - **路由匹配**: bindings 使用 session key 选择 Agent

use crate::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Session Key
///
/// 唯一标识一个会话，格式: `{channel_type}:{channel_user_id}[:{team_id}]`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey {
    /// 通道类型
    pub channel_type: String,
    /// 通道用户 ID
    pub channel_user_id: String,
    /// 团队 ID（可选，用于 Slack/Discord 工作区）
    pub team_id: Option<String>,
}

impl SessionKey {
    /// 创建新的 Session Key
    pub fn new(
        channel_type: impl Into<String>,
        channel_user_id: impl Into<String>,
    ) -> Self {
        Self {
            channel_type: channel_type.into(),
            channel_user_id: channel_user_id.into(),
            team_id: None,
        }
    }

    /// 创建带团队 ID 的 Session Key
    pub fn with_team(
        channel_type: impl Into<String>,
        channel_user_id: impl Into<String>,
        team_id: impl Into<String>,
    ) -> Self {
        Self {
            channel_type: channel_type.into(),
            channel_user_id: channel_user_id.into(),
            team_id: Some(team_id.into()),
        }
    }

    /// 从字符串解析 Session Key
    ///
    /// 支持格式:
    /// - `telegram:user123`
    /// - `slack:user456:T123`
    pub fn parse(s: &str) -> AgentResult<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.len() {
            2 => Ok(Self {
                channel_type: parts[0].to_string(),
                channel_user_id: parts[1].to_string(),
                team_id: None,
            }),
            3 => Ok(Self {
                channel_type: parts[0].to_string(),
                channel_user_id: parts[1].to_string(),
                team_id: Some(parts[2].to_string()),
            }),
            _ => Err(AgentError::InvalidConfig(format!(
                "Invalid session key format: '{}'. Expected: channel_type:user_id[:team_id]",
                s
            ))),
        }
    }

    /// 获取字符串表示
    pub fn as_str(&self) -> String {
        if let Some(team_id) = &self.team_id {
            format!("{}:{}:{}", self.channel_type, self.channel_user_id, team_id)
        } else {
            format!("{}:{}", self.channel_type, self.channel_user_id)
        }
    }

    /// 是否匹配指定的 channel 类型
    pub fn matches_channel(&self, channel_type: &str) -> bool {
        self.channel_type == channel_type
    }

    /// 是否匹配指定的 team
    pub fn matches_team(&self, team_id: &str) -> bool {
        self.team_id.as_ref().map(|t| t == team_id).unwrap_or(false)
    }

    /// 获取用户 ID（不带 team）
    pub fn user_key(&self) -> String {
        format!("{}:{}", self.channel_type, self.channel_user_id)
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 通道类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Telegram,
    Slack,
    Discord,
    WhatsApp,
    DingTalk,
    Feishu,
    WeWork,
    Signal,
    iMessage,
    Web,
    Custom(String),
}

impl ChannelType {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "telegram" => ChannelType::Telegram,
            "slack" => ChannelType::Slack,
            "discord" => ChannelType::Discord,
            "whatsapp" => ChannelType::WhatsApp,
            "dingtalk" | "钉钉" => ChannelType::DingTalk,
            "feishu" | "飞书" => ChannelType::Feishu,
            "wework" | "企业微信" | "wecom" => ChannelType::WeWork,
            "signal" => ChannelType::Signal,
            "imessage" | "imesg" => ChannelType::iMessage,
            "web" => ChannelType::Web,
            other => ChannelType::Custom(other.to_string()),
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &str {
        match self {
            ChannelType::Telegram => "telegram",
            ChannelType::Slack => "slack",
            ChannelType::Discord => "discord",
            ChannelType::WhatsApp => "whatsapp",
            ChannelType::DingTalk => "dingtalk",
            ChannelType::Feishu => "feishu",
            ChannelType::WeWork => "wework",
            ChannelType::Signal => "signal",
            ChannelType::iMessage => "imessage",
            ChannelType::Web => "web",
            ChannelType::Custom(s) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_key_parse_simple() {
        let key = SessionKey::parse("telegram:user123").unwrap();
        assert_eq!(key.channel_type, "telegram");
        assert_eq!(key.channel_user_id, "user123");
        assert!(key.team_id.is_none());
    }

    #[test]
    fn test_session_key_parse_with_team() {
        let key = SessionKey::parse("slack:user456:T123").unwrap();
        assert_eq!(key.channel_type, "slack");
        assert_eq!(key.channel_user_id, "user456");
        assert_eq!(key.team_id, Some("T123".to_string()));
    }

    #[test]
    fn test_session_key_as_str() {
        let key = SessionKey::new("telegram", "user123");
        assert_eq!(key.as_str(), "telegram:user123");

        let key_with_team = SessionKey::with_team("slack", "user456", "T123");
        assert_eq!(key_with_team.as_str(), "slack:user456:T123");
    }

    #[test]
    fn test_session_key_matches() {
        let key = SessionKey::with_team("slack", "user456", "T123");
        assert!(key.matches_channel("slack"));
        assert!(key.matches_team("T123"));
        assert!(!key.matches_team("T456"));
    }
}
