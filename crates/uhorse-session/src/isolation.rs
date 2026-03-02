//! # 隔离策略
//!
//! 定义会话之间的隔离策略。

use uhorse_core::{SessionId, IsolationLevel, Session};
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// 隔离策略
#[derive(Debug, Clone)]
pub struct IsolationPolicy {
    level: IsolationLevel,
}

impl IsolationPolicy {
    pub fn new(level: IsolationLevel) -> Self {
        Self { level }
    }

    /// 检查两个会话是否可以共享上下文
    pub fn can_share_context(&self, session_a: &Session, session_b: &Session) -> bool {
        match self.level {
            IsolationLevel::None => true,
            IsolationLevel::Channel => session_a.channel == session_b.channel,
            IsolationLevel::User => session_a.channel_user_id == session_b.channel_user_id,
            IsolationLevel::Full => session_a.id == session_b.id,
        }
    }

    /// 获取隔离组键
    pub fn get_isolation_group(&self, session: &Session) -> String {
        match self.level {
            IsolationLevel::None => "global".to_string(),
            IsolationLevel::Channel => format!("channel:{}", session.channel),
            IsolationLevel::User => format!("{}:{}", session.channel, session.channel_user_id),
            IsolationLevel::Full => format!("session:{}", session.id),
        }
    }
}

/// 隔离上下文管理器
#[derive(Debug)]
pub struct IsolationContext {
    policy: IsolationPolicy,
    groups: HashMap<String, HashSet<SessionId>>,
}

impl IsolationContext {
    pub fn new(level: IsolationLevel) -> Self {
        Self {
            policy: IsolationPolicy::new(level),
            groups: HashMap::new(),
        }
    }

    /// 注册会话到隔离组
    pub fn register(&mut self, session: &Session) {
        let group_key = self.policy.get_isolation_group(session);
        debug!("Registering session {} to group: {}", session.id, group_key);

        self.groups
            .entry(group_key)
            .or_insert_with(HashSet::new)
            .insert(session.id.clone());
    }

    /// 注销会话
    pub fn unregister(&mut self, session: &Session) {
        let group_key = self.policy.get_isolation_group(session);
        if let Some(group) = self.groups.get_mut(&group_key) {
            group.remove(&session.id);
            if group.is_empty() {
                self.groups.remove(&group_key);
            }
        }
    }

    /// 获取同组的所有会话
    pub fn get_group_members(&self, session: &Session) -> Vec<SessionId> {
        let group_key = self.policy.get_isolation_group(session);
        self.groups
            .get(&group_key)
            .map(|group| group.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 检查两个会话是否在同一组
    pub fn is_same_group(&self, session_a: &Session, session_b: &Session) -> bool {
        self.policy.can_share_context(session_a, session_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uhorse_core::ChannelType;

    #[test]
    fn test_no_isolation() {
        let policy = IsolationPolicy::new(IsolationLevel::None);
        let session_a = Session::new(ChannelType::Telegram, "user1".to_string());
        let session_b = Session::new(ChannelType::Slack, "user2".to_string());

        assert!(policy.can_share_context(&session_a, &session_b));
    }

    #[test]
    fn test_channel_isolation() {
        let policy = IsolationPolicy::new(IsolationLevel::Channel);
        let session_a = Session::new(ChannelType::Telegram, "user1".to_string());
        let session_b = Session::new(ChannelType::Telegram, "user2".to_string());
        let session_c = Session::new(ChannelType::Slack, "user1".to_string());

        assert!(policy.can_share_context(&session_a, &session_b));
        assert!(!policy.can_share_context(&session_a, &session_c));
    }

    #[test]
    fn test_user_isolation() {
        let policy = IsolationPolicy::new(IsolationLevel::User);
        let session_a = Session::new(ChannelType::Telegram, "user1".to_string());
        let session_b = Session::new(ChannelType::Telegram, "user1".to_string());
        let session_c = Session::new(ChannelType::Telegram, "user2".to_string());

        assert!(policy.can_share_context(&session_a, &session_b));
        assert!(!policy.can_share_context(&session_a, &session_c));
    }

    #[test]
    fn test_full_isolation() {
        let policy = IsolationPolicy::new(IsolationLevel::Full);
        let session_a = Session::new(ChannelType::Telegram, "user1".to_string());

        // 相同会话应该可以共享
        assert!(policy.can_share_context(&session_a, &session_a));
    }
}
