//! # Bindings - Agent 路由绑定
//!
//! OpenClaw 风格的 Bindings 路由系统，将消息路由到合适的 Agent。
//!
//! ## 配置示例
//!
//! ```json
//! {
//!   "bindings": [
//!     { "agent": "main", "channel": "telegram" },
//!     { "agent": "coder", "channel": "slack", "teamId": "T123" },
//!     { "agent": "writer", "channel": "discord" }
//!   ]
//! }
//! ```
//!
//! ## 路由优先级
//!
//! 1. **精确匹配**: channel + teamId 完全匹配
//! 2. **通道匹配**: 仅 channel 匹配
//! 3. **默认**: 使用默认 Agent

use crate::error::{AgentError, AgentResult};
use crate::session_key::SessionKey;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Binding 配置
///
/// 定义消息到 Agent 的路由规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    /// 目标 Agent ID
    pub agent: String,
    /// 通道类型（如 "telegram", "slack"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// 团队 ID（用于 Slack/Discord 工作区）
    #[serde(rename = "teamId", skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// 优先级（数字越大优先级越高）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// 用户 ID 白名单
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<String>>,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Binding {
    /// 创建新的 Binding
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            channel: None,
            team_id: None,
            priority: None,
            users: None,
            enabled: true,
        }
    }

    /// 设置通道
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// 设置团队 ID
    pub fn team_id(mut self, team_id: impl Into<String>) -> Self {
        self.team_id = Some(team_id.into());
        self
    }

    /// 设置优先级
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// 设置用户白名单
    pub fn users(mut self, users: Vec<String>) -> Self {
        self.users = Some(users);
        self
    }

    /// 检查是否匹配指定的 Session Key
    pub fn matches(&self, session_key: &SessionKey) -> bool {
        // 检查是否启用
        if !self.enabled {
            return false;
        }

        // 检查通道匹配
        if let Some(channel) = &self.channel {
            if !session_key.matches_channel(channel) {
                return false;
            }
        }

        // 检查团队 ID 匹配
        if let Some(team_id) = &self.team_id {
            if !session_key.matches_team(team_id) {
                return false;
            }
        }

        // 检查用户白名单
        if let Some(users) = &self.users {
            if !users.contains(&session_key.channel_user_id) {
                return false;
            }
        }

        true
    }

    /// 获取优先级
    pub fn get_priority(&self) -> i32 {
        self.priority.unwrap_or(0)
    }
}

/// Bindings 配置
///
/// 管理所有 Agent 路由绑定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingsConfig {
    /// 绑定列表
    #[serde(default)]
    pub bindings: Vec<Binding>,
    /// 默认 Agent（当没有匹配时使用）
    #[serde(default = "default_agent")]
    pub default_agent: String,
}

fn default_agent() -> String {
    "main".to_string()
}

impl Default for BindingsConfig {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            default_agent: "main".to_string(),
        }
    }
}

impl BindingsConfig {
    /// 创建新的 Bindings 配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加绑定
    pub fn add_binding(&mut self, binding: Binding) {
        self.bindings.push(binding);
    }

    /// 设置默认 Agent
    pub fn default_agent(mut self, agent: impl Into<String>) -> Self {
        self.default_agent = agent.into();
        self
    }
}

/// Bindings 路由器
///
/// 根据 Session Key 路由消息到合适的 Agent。
pub struct BindingsRouter {
    /// 配置
    config: Arc<BindingsConfig>,
    /// 可用的 Agent ID 列表
    available_agents: Vec<String>,
}

impl BindingsRouter {
    /// 创建新的路由器
    pub fn new(config: BindingsConfig, available_agents: Vec<String>) -> Self {
        Self {
            config: Arc::new(config),
            available_agents,
        }
    }

    /// 路由到 Agent
    ///
    /// 根据 Session Key 返回应该处理的 Agent ID。
    pub fn route(&self, session_key: &SessionKey) -> AgentResult<String> {
        // 找到所有匹配的绑定
        let mut matched: Vec<&Binding> = self
            .config
            .bindings
            .iter()
            .filter(|b| b.matches(session_key))
            .collect();

        if matched.is_empty() {
            // 没有匹配，使用默认 Agent
            return Ok(self.config.default_agent.clone());
        }

        // 按优先级排序（优先级高的在前）
        matched.sort_by_key(|binding| std::cmp::Reverse(binding.get_priority()));

        // 返回优先级最高的匹配
        let chosen = matched.first().unwrap();
        let agent_id = &chosen.agent;

        // 验证 Agent 是否可用
        if !self.available_agents.contains(agent_id) {
            return Err(AgentError::Agent(format!(
                "Agent '{}' is not available",
                agent_id
            )));
        }

        Ok(agent_id.clone())
    }

    /// 获取配置的引用
    pub fn config(&self) -> &BindingsConfig {
        &self.config
    }
}

/// Binding 构建器
///
/// 方便创建 Binding 配置。
pub struct BindingBuilder {
    binding: Binding,
}

impl BindingBuilder {
    /// 创建新的构建器
    pub fn new(agent: impl Into<String>) -> Self {
        Self {
            binding: Binding::new(agent),
        }
    }

    /// 设置通道
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.binding.channel = Some(channel.into());
        self
    }

    /// 设置团队 ID
    pub fn team_id(mut self, team_id: impl Into<String>) -> Self {
        self.binding.team_id = Some(team_id.into());
        self
    }

    /// 设置优先级
    pub fn priority(mut self, priority: i32) -> Self {
        self.binding.priority = Some(priority);
        self
    }

    /// 设置用户白名单
    pub fn users(mut self, users: Vec<String>) -> Self {
        self.binding.users = Some(users);
        self
    }

    /// 构建
    pub fn build(self) -> Binding {
        self.binding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_matches() {
        let binding = BindingBuilder::new("coder")
            .channel("slack")
            .team_id("T123")
            .build();

        let session_key = SessionKey::with_team("slack", "user456", "T123");
        assert!(binding.matches(&session_key));

        let wrong_team = SessionKey::with_team("slack", "user456", "T456");
        assert!(!binding.matches(&wrong_team));

        let wrong_channel = SessionKey::new("telegram", "user456");
        assert!(!binding.matches(&wrong_channel));
    }

    #[test]
    fn test_router() {
        let mut config = BindingsConfig::new();
        config.add_binding(
            BindingBuilder::new("coder")
                .channel("slack")
                .team_id("T123")
                .build(),
        );
        config.add_binding(BindingBuilder::new("main").channel("telegram").build());

        let router = BindingsRouter::new(config, vec!["main".to_string(), "coder".to_string()]);

        // 匹配 coder
        let session_key = SessionKey::with_team("slack", "user456", "T123");
        assert_eq!(router.route(&session_key).unwrap(), "coder");

        // 匹配 main
        let session_key = SessionKey::new("telegram", "user789");
        assert_eq!(router.route(&session_key).unwrap(), "main");

        // 使用默认
        let session_key = SessionKey::new("discord", "user999");
        assert_eq!(router.route(&session_key).unwrap(), "main");
    }

    #[test]
    fn test_priority_routing() {
        let mut config = BindingsConfig::new();
        config.add_binding(
            BindingBuilder::new("agent1")
                .channel("slack")
                .priority(10)
                .build(),
        );
        config.add_binding(
            BindingBuilder::new("agent2")
                .channel("slack")
                .priority(20)
                .build(),
        );

        let router = BindingsRouter::new(config, vec!["agent1".to_string(), "agent2".to_string()]);

        let session_key = SessionKey::new("slack", "user123");
        // agent2 优先级更高
        assert_eq!(router.route(&session_key).unwrap(), "agent2");
    }
}
