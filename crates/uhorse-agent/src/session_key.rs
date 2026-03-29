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

/// 访问上下文。
///
/// 在不改变 `SessionKey` 字符串格式的前提下，为 session 补充企业级共享链所需的
/// `tenant / enterprise / department / role` 作用域信息。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessContext {
    /// 可选租户作用域键。
    #[serde(default)]
    pub tenant: Option<String>,
    /// 可选企业作用域键。
    #[serde(default)]
    pub enterprise: Option<String>,
    /// 可选部门作用域键。
    #[serde(default)]
    pub department: Option<String>,
    /// 可选角色作用域键集合。
    #[serde(default)]
    pub roles: Vec<String>,
}

impl AccessContext {
    /// 创建空访问上下文。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回规范化后的访问上下文。
    pub fn normalized(mut self) -> Self {
        self.tenant = normalize_optional_scope(self.tenant);
        self.enterprise = normalize_optional_scope(self.enterprise);
        self.department = normalize_optional_scope(self.department);
        self.roles = normalize_role_scopes(self.roles);
        self
    }
}

/// 会话运行时命名空间。
///
/// 在不改变 `SessionKey` 字符串格式的前提下，提供统一的
/// `global / tenant / enterprise / department / role / user / session` 作用域键。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionNamespace {
    /// 全局共享作用域键。
    pub global: String,
    /// 租户共享作用域键。
    pub tenant: Option<String>,
    /// 企业共享作用域键。
    #[serde(default)]
    pub enterprise: Option<String>,
    /// 部门共享作用域键。
    #[serde(default)]
    pub department: Option<String>,
    /// 角色共享作用域键。
    #[serde(default)]
    pub roles: Vec<String>,
    /// 用户私有作用域键。
    pub user: String,
    /// 会话私有作用域键。
    pub session: String,
}

impl SessionNamespace {
    /// 根据 SessionKey 生成基础命名空间。
    pub fn from_session_key(session_key: &SessionKey) -> Self {
        Self {
            global: "global".to_string(),
            tenant: session_key.tenant_key(),
            enterprise: None,
            department: None,
            roles: Vec::new(),
            user: format!("user:{}", session_key.user_key()),
            session: format!("session:{}", session_key.as_str()),
        }
    }

    /// 使用访问上下文补充企业级作用域信息。
    pub fn with_access_context(mut self, access_context: Option<&AccessContext>) -> Self {
        let Some(access_context) = access_context.cloned().map(AccessContext::normalized) else {
            return self;
        };

        if self.tenant.is_none() {
            self.tenant = access_context.tenant;
        }
        self.enterprise = access_context.enterprise;
        self.department = access_context.department;
        self.roles = access_context.roles;
        self
    }

    /// 返回 memory 上下文从共享到私有的读取顺序。
    pub fn memory_context_chain(&self) -> Vec<String> {
        let mut scopes = vec![self.global.clone()];
        if let Some(tenant) = &self.tenant {
            scopes.push(tenant.clone());
        }
        if let Some(enterprise) = &self.enterprise {
            scopes.push(enterprise.clone());
        }
        if let Some(department) = &self.department {
            scopes.push(department.clone());
        }
        scopes.extend(self.roles.iter().cloned());
        scopes.push(self.user.clone());
        scopes.push(self.session.clone());
        scopes
    }

    /// 返回 agent / skill 从私有到共享的解析顺序。
    pub fn visibility_chain(&self) -> Vec<String> {
        let mut scopes = vec![self.user.clone()];
        scopes.extend(self.roles.iter().cloned());
        if let Some(department) = &self.department {
            scopes.push(department.clone());
        }
        if let Some(enterprise) = &self.enterprise {
            scopes.push(enterprise.clone());
        }
        if let Some(tenant) = &self.tenant {
            scopes.push(tenant.clone());
        }
        scopes.push(self.global.clone());
        scopes
    }
}

fn normalize_optional_scope(scope: Option<String>) -> Option<String> {
    scope.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_role_scopes(roles: Vec<String>) -> Vec<String> {
    let mut normalized = roles
        .into_iter()
        .filter_map(|role| {
            let trimmed = role.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// 根据作用域键推导层级名称。
pub fn scope_layer_from_scope(scope: &str) -> &'static str {
    if scope == "global" {
        "global"
    } else if scope.starts_with("session:") {
        "session"
    } else if scope.starts_with("user:") {
        "user"
    } else if scope.starts_with("role:") {
        "role"
    } else if scope.starts_with("department:") {
        "department"
    } else if scope.starts_with("enterprise:") {
        "enterprise"
    } else if scope.starts_with("tenant:") {
        "tenant"
    } else {
        "scope"
    }
}

/// 返回作用域层级的优先级，值越小优先级越高。
pub fn scope_layer_rank(layer: &str) -> usize {
    match layer {
        "session" => 0,
        "user" => 1,
        "role" => 2,
        "department" => 3,
        "enterprise" => 4,
        "tenant" => 5,
        "global" => 6,
        _ => 7,
    }
}

impl SessionKey {
    /// 创建新的 Session Key
    pub fn new(channel_type: impl Into<String>, channel_user_id: impl Into<String>) -> Self {
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

    /// 获取租户作用域键。
    pub fn tenant_key(&self) -> Option<String> {
        self.team_id
            .as_ref()
            .map(|team_id| format!("tenant:{}:{}", self.channel_type, team_id))
    }

    /// 获取运行时命名空间。
    pub fn namespace(&self) -> SessionNamespace {
        SessionNamespace::from_session_key(self)
    }

    /// 使用访问上下文生成运行时命名空间。
    pub fn namespace_with_access_context(
        &self,
        access_context: Option<&AccessContext>,
    ) -> SessionNamespace {
        self.namespace().with_access_context(access_context)
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
    /// Telegram 通道。
    Telegram,
    /// Slack 通道。
    Slack,
    /// Discord 通道。
    Discord,
    /// WhatsApp 通道。
    WhatsApp,
    /// 钉钉通道。
    DingTalk,
    /// 飞书通道。
    Feishu,
    /// 企业微信通道。
    WeWork,
    /// Signal 通道。
    Signal,
    /// iMessage 通道。
    IMessage,
    /// Web 通道。
    Web,
    /// 自定义通道。
    Custom(
        /// 自定义通道名称。
        String,
    ),
}

impl std::str::FromStr for ChannelType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "telegram" => ChannelType::Telegram,
            "slack" => ChannelType::Slack,
            "discord" => ChannelType::Discord,
            "whatsapp" => ChannelType::WhatsApp,
            "dingtalk" | "钉钉" => ChannelType::DingTalk,
            "feishu" | "飞书" => ChannelType::Feishu,
            "wework" | "企业微信" | "wecom" => ChannelType::WeWork,
            "signal" => ChannelType::Signal,
            "imessage" | "imesg" => ChannelType::IMessage,
            "web" => ChannelType::Web,
            other => ChannelType::Custom(other.to_string()),
        })
    }
}

impl ChannelType {
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
            ChannelType::IMessage => "imessage",
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

    #[test]
    fn test_session_namespace_without_team() {
        let key = SessionKey::new("telegram", "user123");
        let namespace = key.namespace();

        assert_eq!(namespace.global, "global");
        assert!(namespace.tenant.is_none());
        assert!(namespace.enterprise.is_none());
        assert!(namespace.department.is_none());
        assert!(namespace.roles.is_empty());
        assert_eq!(namespace.user, "user:telegram:user123");
        assert_eq!(namespace.session, "session:telegram:user123");
        assert_eq!(
            namespace.memory_context_chain(),
            vec![
                "global".to_string(),
                "user:telegram:user123".to_string(),
                "session:telegram:user123".to_string()
            ]
        );
        assert_eq!(
            namespace.visibility_chain(),
            vec!["user:telegram:user123".to_string(), "global".to_string()]
        );
    }

    #[test]
    fn test_session_namespace_with_team() {
        let key = SessionKey::with_team("dingtalk", "user456", "corp789");
        let namespace = key.namespace();

        assert_eq!(
            key.tenant_key(),
            Some("tenant:dingtalk:corp789".to_string())
        );
        assert_eq!(namespace.global, "global");
        assert_eq!(
            namespace.tenant,
            Some("tenant:dingtalk:corp789".to_string())
        );
        assert!(namespace.enterprise.is_none());
        assert!(namespace.department.is_none());
        assert!(namespace.roles.is_empty());
        assert_eq!(namespace.user, "user:dingtalk:user456");
        assert_eq!(namespace.session, "session:dingtalk:user456:corp789");
        assert_eq!(
            namespace.memory_context_chain(),
            vec![
                "global".to_string(),
                "tenant:dingtalk:corp789".to_string(),
                "user:dingtalk:user456".to_string(),
                "session:dingtalk:user456:corp789".to_string()
            ]
        );
        assert_eq!(
            namespace.visibility_chain(),
            vec![
                "user:dingtalk:user456".to_string(),
                "tenant:dingtalk:corp789".to_string(),
                "global".to_string()
            ]
        );
    }

    #[test]
    fn test_session_namespace_with_access_context_extends_enterprise_chain() {
        let key = SessionKey::with_team("dingtalk", "user456", "corp789");
        let namespace = key.namespace_with_access_context(Some(&AccessContext {
            tenant: Some("tenant:override:ignored".to_string()),
            enterprise: Some("enterprise:org-1".to_string()),
            department: Some("department:org-1:sales".to_string()),
            roles: vec![
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string(),
            ],
        }));

        assert_eq!(
            namespace.tenant,
            Some("tenant:dingtalk:corp789".to_string())
        );
        assert_eq!(namespace.enterprise.as_deref(), Some("enterprise:org-1"));
        assert_eq!(
            namespace.department.as_deref(),
            Some("department:org-1:sales")
        );
        assert_eq!(
            namespace.roles,
            vec![
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string()
            ]
        );
        assert_eq!(
            namespace.memory_context_chain(),
            vec![
                "global".to_string(),
                "tenant:dingtalk:corp789".to_string(),
                "enterprise:org-1".to_string(),
                "department:org-1:sales".to_string(),
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string(),
                "user:dingtalk:user456".to_string(),
                "session:dingtalk:user456:corp789".to_string()
            ]
        );
        assert_eq!(
            namespace.visibility_chain(),
            vec![
                "user:dingtalk:user456".to_string(),
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string(),
                "department:org-1:sales".to_string(),
                "enterprise:org-1".to_string(),
                "tenant:dingtalk:corp789".to_string(),
                "global".to_string()
            ]
        );
    }

    #[test]
    fn test_access_context_normalizes_roles_and_empty_values() {
        let access_context = AccessContext {
            tenant: Some(" ".to_string()),
            enterprise: Some(" enterprise:org-1 ".to_string()),
            department: Some("department:org-1:sales".to_string()),
            roles: vec![
                "role:org-1:manager".to_string(),
                " ".to_string(),
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string(),
            ],
        }
        .normalized();

        assert!(access_context.tenant.is_none());
        assert_eq!(
            access_context.enterprise.as_deref(),
            Some("enterprise:org-1")
        );
        assert_eq!(
            access_context.roles,
            vec![
                "role:org-1:approver".to_string(),
                "role:org-1:manager".to_string()
            ]
        );
    }
}
