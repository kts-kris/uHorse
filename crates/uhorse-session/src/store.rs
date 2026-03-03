//! # 会话存储适配器
//!
//! 为 SessionStore 提供适配器实现。

use std::sync::Arc;
use uhorse_core::{Result, SessionStore};
use uhorse_storage::SqliteStore;

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
    async fn create_session(&self, session: &uhorse_core::Session) -> Result<()> {
        self.store.create_session(session).await
    }

    async fn get_session(
        &self,
        id: &uhorse_core::SessionId,
    ) -> Result<Option<uhorse_core::Session>> {
        self.store.get_session(id).await
    }

    async fn get_session_by_channel(
        &self,
        channel: uhorse_core::ChannelType,
        channel_user_id: &str,
    ) -> Result<Option<uhorse_core::Session>> {
        self.store
            .get_session_by_channel(channel, channel_user_id)
            .await
    }

    async fn update_session(&self, session: &uhorse_core::Session) -> Result<()> {
        self.store.update_session(session).await
    }

    async fn delete_session(&self, id: &uhorse_core::SessionId) -> Result<()> {
        self.store.delete_session(id).await
    }

    async fn list_sessions(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<uhorse_core::Session>> {
        self.store.list_sessions(limit, offset).await
    }
}
