//! # 会话存储适配器
//!
//! 为 SessionStore 提供适配器实现。

use openclaw_core::{SessionStore, Result};
use openclaw_storage::SqliteStore;
use std::sync::Arc;

/// 会话存储适配器
#[derive(Debug)]
pub struct SessionStoreAdapter {
    store: Arc<SqliteStore>,
}

impl SessionStoreAdapter {
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl SessionStore for SessionStoreAdapter {
    async fn create_session(&self, session: &openclaw_core::Session) -> Result<()> {
        self.store.create_session(session).await
    }

    async fn get_session(&self, id: &openclaw_core::SessionId) -> Result<Option<openclaw_core::Session>> {
        self.store.get_session(id).await
    }

    async fn get_session_by_channel(&self, channel: openclaw_core::ChannelType, channel_user_id: &str) -> Result<Option<openclaw_core::Session>> {
        self.store.get_session_by_channel(channel, channel_user_id).await
    }

    async fn update_session(&self, session: &openclaw_core::Session) -> Result<()> {
        self.store.update_session(session).await
    }

    async fn delete_session(&self, id: &openclaw_core::SessionId) -> Result<()> {
        self.store.delete_session(id).await
    }

    async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<openclaw_core::Session>> {
        self.store.list_sessions(limit, offset).await
    }
}
