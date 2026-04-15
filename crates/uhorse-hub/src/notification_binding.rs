//! 通知绑定管理器
//!
//! 负责维护 Hub 运行时的节点通知接收人绑定关系。
//! 静态 `notification_bindings` 作为 seed/fallback 导入，运行时绑定优先级更高。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use uhorse_config::DingTalkNotificationBinding;
use uhorse_core::{ChannelRecipient, ChannelType};

/// 通用通知渠道绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationBinding {
    /// 绑定的消息渠道类型。
    pub channel_type: ChannelType,
    /// 渠道内接收人标识。
    pub recipient: String,
}

impl NotificationBinding {
    /// 转为通用通道收件人。
    pub fn as_recipient(&self) -> ChannelRecipient {
        ChannelRecipient {
            channel_type: self.channel_type,
            recipient: self.recipient.clone(),
        }
    }
}

/// 通知绑定管理器。
#[derive(Debug, Clone)]
pub struct NotificationBindingManager {
    seed_bindings: Arc<HashMap<String, NotificationBinding>>,
    runtime_bindings: Arc<RwLock<HashMap<String, Option<NotificationBinding>>>>,
}

impl NotificationBindingManager {
    /// 使用静态 seed 绑定创建管理器。
    pub fn new(seed_bindings: Vec<DingTalkNotificationBinding>) -> Self {
        Self {
            seed_bindings: Arc::new(
                seed_bindings
                    .into_iter()
                    .map(|binding| {
                        (
                            binding.node_id,
                            NotificationBinding {
                                channel_type: ChannelType::DingTalk,
                                recipient: binding.user_id,
                            },
                        )
                    })
                    .collect(),
            ),
            runtime_bindings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 查询节点当前生效的通知绑定。
    pub async fn get_binding(&self, node_id: &str) -> Option<NotificationBinding> {
        let runtime_bindings = self.runtime_bindings.read().await;
        if let Some(binding) = runtime_bindings.get(node_id) {
            return binding.clone();
        }

        self.seed_bindings.get(node_id).cloned()
    }

    /// 查询节点当前生效的 DingTalk 接收人。
    pub async fn get_user_id(&self, node_id: &str) -> Option<String> {
        self.get_binding(node_id)
            .await
            .filter(|binding| binding.channel_type == ChannelType::DingTalk)
            .map(|binding| binding.recipient)
    }

    /// 设置运行时绑定，覆盖静态 seed。
    pub async fn set_binding(&self, node_id: impl Into<String>, user_id: impl Into<String>) {
        self.set_channel_binding(node_id, ChannelType::DingTalk, user_id)
            .await;
    }

    /// 设置运行时渠道绑定，覆盖静态 seed。
    pub async fn set_channel_binding(
        &self,
        node_id: impl Into<String>,
        channel_type: ChannelType,
        recipient: impl Into<String>,
    ) {
        self.runtime_bindings.write().await.insert(
            node_id.into(),
            Some(NotificationBinding {
                channel_type,
                recipient: recipient.into(),
            }),
        );
    }

    /// 清除运行时覆盖，恢复为静态 seed（如果存在）。
    pub async fn clear_runtime_binding(&self, node_id: &str) {
        self.runtime_bindings.write().await.remove(node_id);
    }

    /// 显式解绑节点。
    ///
    /// 即使存在静态 seed，也会在当前进程内屏蔽该 seed。
    pub async fn unbind(&self, node_id: impl Into<String>) {
        self.runtime_bindings.write().await.insert(node_id.into(), None);
    }

    /// 返回当前 seed 绑定数量。
    pub fn seed_count(&self) -> usize {
        self.seed_bindings.len()
    }
}

impl Default for NotificationBindingManager {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(node_id: &str, user_id: &str) -> DingTalkNotificationBinding {
        DingTalkNotificationBinding {
            node_id: node_id.to_string(),
            user_id: user_id.to_string(),
        }
    }

    #[tokio::test]
    async fn get_binding_returns_generic_channel_binding() {
        let manager = NotificationBindingManager::new(vec![binding("node-1", "user-seed")]);

        assert_eq!(
            manager.get_binding("node-1").await,
            Some(NotificationBinding {
                channel_type: ChannelType::DingTalk,
                recipient: "user-seed".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn uses_seed_binding_by_default() {
        let manager = NotificationBindingManager::new(vec![binding("node-1", "user-seed")]);

        assert_eq!(
            manager.get_user_id("node-1").await.as_deref(),
            Some("user-seed")
        );
    }

    #[tokio::test]
    async fn runtime_binding_overrides_seed_binding() {
        let manager = NotificationBindingManager::new(vec![binding("node-1", "user-seed")]);

        manager.set_binding("node-1", "user-runtime").await;

        assert_eq!(
            manager.get_user_id("node-1").await.as_deref(),
            Some("user-runtime")
        );
        assert_eq!(
            manager.get_binding("node-1").await,
            Some(NotificationBinding {
                channel_type: ChannelType::DingTalk,
                recipient: "user-runtime".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn clearing_runtime_binding_restores_seed_binding() {
        let manager = NotificationBindingManager::new(vec![binding("node-1", "user-seed")]);

        manager.set_binding("node-1", "user-runtime").await;
        manager.clear_runtime_binding("node-1").await;

        assert_eq!(
            manager.get_user_id("node-1").await.as_deref(),
            Some("user-seed")
        );
    }

    #[tokio::test]
    async fn unbind_blocks_seed_fallback() {
        let manager = NotificationBindingManager::new(vec![binding("node-1", "user-seed")]);

        manager.unbind("node-1").await;

        assert_eq!(manager.get_user_id("node-1").await, None);
        assert_eq!(manager.get_binding("node-1").await, None);
    }

    #[tokio::test]
    async fn set_channel_binding_supports_generic_channel_type() {
        let manager = NotificationBindingManager::default();

        manager
            .set_channel_binding("node-1", ChannelType::Slack, "slack-user-1")
            .await;

        assert_eq!(
            manager.get_binding("node-1").await,
            Some(NotificationBinding {
                channel_type: ChannelType::Slack,
                recipient: "slack-user-1".to_string(),
            })
        );
        assert_eq!(manager.get_user_id("node-1").await, None);
    }
}
