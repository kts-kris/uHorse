//! # 会话管理器
//!
//! 管理会话的创建、获取、更新和销毁。

use uhorse_core::{
    SessionStore, ConversationStore, Result, UHorseError,
    Session, SessionId, ChannelType, Message, MessageContent, MessageRole,
};
use uhorse_storage::SqliteStore;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// 会话管理器
#[derive(Debug, Clone)]
pub struct SessionManager {
    store: Arc<SqliteStore>,
    // 内存缓存（可选，用于提高性能）
    cache: Arc<RwLock<Option<SessionCache>>>,
}

#[derive(Debug)]
struct SessionCache {
    sessions: std::collections::HashMap<SessionId, Session>,
    max_size: usize,
}

impl SessionCache {
    fn new(max_size: usize) -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
            max_size,
        }
    }

    fn get(&self, id: &SessionId) -> Option<&Session> {
        self.sessions.get(id)
    }

    fn insert(&mut self, session: Session) {
        // 如果超过最大大小，移除最旧的条目
        if self.sessions.len() >= self.max_size {
            // 简单的 LRU：移除第一个
            if let Some(key) = self.sessions.keys().next().cloned() {
                self.sessions.remove(&key);
            }
        }
        self.sessions.insert(session.id.clone(), session);
    }

    fn remove(&mut self, id: &SessionId) -> Option<Session> {
        self.sessions.remove(id)
    }
}

impl SessionManager {
    /// 创建新的会话管理器
    pub async fn new(store: Arc<SqliteStore>) -> Result<Self> {
        info!("Creating SessionManager");

        Ok(Self {
            store,
            cache: Arc::new(RwLock::new(None)),
        })
    }

    /// 启用内存缓存
    pub async fn enable_cache(&self, max_size: usize) {
        *self.cache.write().await = Some(SessionCache::new(max_size));
        info!("Session cache enabled with max_size={}", max_size);
    }

    /// 获取或创建会话
    #[instrument(skip(self))]
    pub async fn get_or_create(&self, channel: ChannelType, channel_user_id: String) -> Result<Session> {
        // 首先尝试从数据库获取
        if let Some(session) = self.store.get_session_by_channel(channel, &channel_user_id).await? {
            debug!("Found existing session: {}", session.id);
            return Ok(session);
        }

        // 创建新会话
        self.create(channel, channel_user_id).await
    }

    /// 创建新会话
    #[instrument(skip(self))]
    pub async fn create(&self, channel: ChannelType, channel_user_id: String) -> Result<Session> {
        let mut session = Session::new(channel, channel_user_id);

        self.store.create_session(&session).await?;

        // 更新缓存
        if let Some(cache) = self.cache.write().await.as_mut() {
            cache.insert(session.clone());
        }

        info!("Created new session: {}", session.id);

        Ok(session)
    }

    /// 获取会话
    #[instrument(skip(self))]
    pub async fn get(&self, id: &SessionId) -> Result<Session> {
        // 先检查缓存
        if let Some(cache) = self.cache.read().await.as_ref() {
            if let Some(session) = cache.get(id) {
                return Ok(session.clone());
            }
        }

        // 从数据库获取
        let session = self.store.get_session(id).await?
            .ok_or_else(|| UHorseError::SessionNotFound(id.clone()))?;

        // 更新缓存
        if let Some(cache) = self.cache.write().await.as_mut() {
            cache.insert(session.clone());
        }

        Ok(session)
    }

    /// 更新会话
    #[instrument(skip(self))]
    pub async fn update(&self, session: &Session) -> Result<()> {
        self.store.update_session(session).await?;

        // 更新缓存
        if let Some(cache) = self.cache.write().await.as_mut() {
            cache.insert(session.clone());
        }

        Ok(())
    }

    /// 删除会话
    #[instrument(skip(self))]
    pub async fn delete(&self, id: &SessionId) -> Result<()> {
        self.store.delete_session(id).await?;

        // 从缓存移除
        if let Some(cache) = self.cache.write().await.as_mut() {
            cache.remove(id);
        }

        info!("Deleted session: {}", id);

        Ok(())
    }

    /// 添加消息到会话历史
    #[instrument(skip(self))]
    pub async fn add_message(&self, session_id: &SessionId, role: MessageRole, content: MessageContent) -> Result<Message> {
        // 获取当前序号
        let sequence = self.store.get_last_sequence(session_id).await?
            .map(|s| s + 1)
            .unwrap_or(0);

        let message = Message::new(session_id.clone(), role, content, sequence);

        self.store.add_message(&message).await?;

        Ok(message)
    }

    /// 获取会话历史
    #[instrument(skip(self))]
    pub async fn get_history(&self, session_id: &SessionId, limit: usize) -> Result<Vec<Message>> {
        self.store.get_history(session_id, limit, None).await
    }

    /// 清除会话历史
    #[instrument(skip(self))]
    pub async fn clear_history(&self, session_id: &SessionId) -> Result<()> {
        self.store.clear_history(session_id).await?;
        info!("Cleared history for session: {}", session_id);
        Ok(())
    }

    /// 触摸会话（更新时间戳）
    #[instrument(skip(self))]
    pub async fn touch(&self, id: &SessionId) -> Result<()> {
        let mut session = self.get(id).await?;
        session.touch();
        self.update(&session).await
    }

    /// 列出所有会话
    pub async fn list(&self, limit: usize, offset: usize) -> Result<Vec<Session>> {
        self.store.list_sessions(limit, offset).await
    }
}
