//! # SQLite 存储
//!
//! 基于 SQLite 的持久化实现。

use sqlx::{sqlite::SqliteConnectOptions, Pool, Row, SqlitePool};
use tracing::{debug, info, instrument};
use uhorse_core::types::{ChannelType, Message, MessageContent, MessageRole, Session, SessionId};
use uhorse_core::{ConversationStore, Result, SessionStore, StorageError};

/// SQLite 存储实现
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: Pool<sqlx::Sqlite>,
}

impl SqliteStore {
    /// 创建新的 SQLite 存储
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Creating SQLite store: {}", database_url);

        // 确保数据库目录存在
        if let Some(parent) = std::path::Path::new(database_url).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
        }

        let options = SqliteConnectOptions::new()
            .filename(database_url)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        info!("SQLite pool created successfully");

        Ok(Self { pool })
    }

    /// 运行数据库迁移
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations...");

        sqlx::query(
            r#"
            -- 会话表
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                channel TEXT NOT NULL,
                channel_user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                metadata TEXT,
                isolation_level INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_channel ON sessions(channel);

            -- 对话历史表
            CREATE TABLE IF NOT EXISTS conversation_history (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id),
                UNIQUE(session_id, sequence)
            );

            CREATE INDEX IF NOT EXISTS idx_history_session ON conversation_history(session_id);

            -- 幂等性缓存表
            CREATE TABLE IF NOT EXISTS idempotency_cache (
                key TEXT PRIMARY KEY,
                response TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_idempotency_expiry ON idempotency_cache(expires_at);
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::MigrationError(e.to_string()))?;

        info!("Database migrations completed");

        Ok(())
    }

    /// 获取连接池引用
    pub fn pool(&self) -> &Pool<sqlx::Sqlite> {
        &self.pool
    }
}

#[async_trait::async_trait]
impl SessionStore for SqliteStore {
    #[instrument(skip(self))]
    async fn create_session(&self, session: &Session) -> Result<()> {
        debug!("Creating session: {}", session.id);

        let metadata_json = serde_json::to_string(&session.metadata)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO sessions (id, channel, channel_user_id, created_at, updated_at, metadata, isolation_level)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(session.id.as_str())
        .bind(channel_to_string(session.channel))
        .bind(&session.channel_user_id)
        .bind(session.created_at as i64)
        .bind(session.updated_at as i64)
        .bind(metadata_json)
        .bind(session.isolation_level as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_session(&self, id: &SessionId) -> Result<Option<Session>> {
        debug!("Getting session: {}", id);

        let row = sqlx::query(
            "SELECT id, channel, channel_user_id, created_at, updated_at, metadata, isolation_level FROM sessions WHERE id = ?"
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?;

        Ok(row.map(row_to_session))
    }

    #[instrument(skip(self))]
    async fn get_session_by_channel(
        &self,
        channel: ChannelType,
        channel_user_id: &str,
    ) -> Result<Option<Session>> {
        debug!(
            "Getting session by channel: {:?} / {}",
            channel, channel_user_id
        );

        let row = sqlx::query(
            "SELECT id, channel, channel_user_id, created_at, updated_at, metadata, isolation_level FROM sessions WHERE channel = ? AND channel_user_id = ? ORDER BY updated_at DESC LIMIT 1"
        )
        .bind(channel_to_string(channel))
        .bind(channel_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?;

        Ok(row.map(row_to_session))
    }

    #[instrument(skip(self))]
    async fn update_session(&self, session: &Session) -> Result<()> {
        debug!("Updating session: {}", session.id);

        let metadata_json = serde_json::to_string(&session.metadata)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE sessions
            SET channel = ?, channel_user_id = ?, updated_at = ?, metadata = ?, isolation_level = ?
            WHERE id = ?
            "#,
        )
        .bind(channel_to_string(session.channel))
        .bind(&session.channel_user_id)
        .bind(session.updated_at as i64)
        .bind(metadata_json)
        .bind(session.isolation_level as i32)
        .bind(session.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete_session(&self, id: &SessionId) -> Result<()> {
        debug!("Deleting session: {}", id);

        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<Session>> {
        debug!("Listing sessions: limit={}, offset={}", limit, offset);

        let rows = sqlx::query(
            "SELECT id, channel, channel_user_id, created_at, updated_at, metadata, isolation_level FROM sessions ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryError(e.to_string()))?;

        Ok(rows.into_iter().map(row_to_session).collect())
    }
}

#[async_trait::async_trait]
impl ConversationStore for SqliteStore {
    #[instrument(skip(self))]
    async fn add_message(&self, message: &Message) -> Result<()> {
        debug!(
            "Adding message: {} to session: {}",
            message.id, message.session_id
        );

        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO conversation_history (id, session_id, sequence, role, content, timestamp)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&message.id)
        .bind(message.session_id.as_str())
        .bind(message.sequence as i64)
        .bind(role_to_string(message.role))
        .bind(content_json)
        .bind(message.timestamp as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_history(
        &self,
        session_id: &SessionId,
        limit: usize,
        before_sequence: Option<u64>,
    ) -> Result<Vec<Message>> {
        debug!(
            "Getting history for session: {} limit: {} before: {:?}",
            session_id, limit, before_sequence
        );

        let rows = if let Some(before) = before_sequence {
            sqlx::query(
                "SELECT id, session_id, role, content, timestamp, sequence FROM conversation_history WHERE session_id = ? AND sequence < ? ORDER BY sequence DESC LIMIT ?"
            )
            .bind(session_id.as_str())
            .bind(before as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                "SELECT id, session_id, role, content, timestamp, sequence FROM conversation_history WHERE session_id = ? ORDER BY sequence DESC LIMIT ?"
            )
            .bind(session_id.as_str())
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        }.map_err(|e| StorageError::QueryError(e.to_string()))?;

        // 反转顺序（因为查询是 DESC）
        let messages: Vec<Message> = rows.into_iter().rev().map(row_to_message).collect();

        Ok(messages)
    }

    #[instrument(skip(self))]
    async fn get_last_sequence(&self, session_id: &SessionId) -> Result<Option<u64>> {
        debug!("Getting last sequence for session: {}", session_id);

        let row: Option<(i64,)> =
            sqlx::query_as("SELECT MAX(sequence) FROM conversation_history WHERE session_id = ?")
                .bind(session_id.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| StorageError::QueryError(e.to_string()))?;

        Ok(row.map(|(s,)| s as u64))
    }

    #[instrument(skip(self))]
    async fn clear_history(&self, session_id: &SessionId) -> Result<()> {
        debug!("Clearing history for session: {}", session_id);

        sqlx::query("DELETE FROM conversation_history WHERE session_id = ?")
            .bind(session_id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

// ============== 辅助函数 ==============

fn channel_to_string(channel: ChannelType) -> String {
    match channel {
        ChannelType::Telegram => "telegram".to_string(),
        ChannelType::Slack => "slack".to_string(),
        ChannelType::Discord => "discord".to_string(),
        ChannelType::WhatsApp => "whatsapp".to_string(),
        ChannelType::DingTalk => "dingtalk".to_string(),
        ChannelType::Feishu => "feishu".to_string(),
        ChannelType::WeWork => "wework".to_string(),
    }
}

fn string_to_channel(s: &str) -> Option<ChannelType> {
    match s {
        "telegram" => Some(ChannelType::Telegram),
        "slack" => Some(ChannelType::Slack),
        "discord" => Some(ChannelType::Discord),
        "whatsapp" => Some(ChannelType::WhatsApp),
        "dingtalk" => Some(ChannelType::DingTalk),
        "feishu" => Some(ChannelType::Feishu),
        "wework" => Some(ChannelType::WeWork),
        _ => None,
    }
}

fn role_to_string(role: MessageRole) -> String {
    match role {
        MessageRole::User => "User".to_string(),
        MessageRole::Assistant => "Assistant".to_string(),
        MessageRole::System => "System".to_string(),
        MessageRole::Tool => "Tool".to_string(),
    }
}

fn string_to_role(s: &str) -> MessageRole {
    match s {
        "User" => MessageRole::User,
        "Assistant" => MessageRole::Assistant,
        "System" => MessageRole::System,
        "Tool" => MessageRole::Tool,
        _ => MessageRole::User,
    }
}

fn row_to_session(row: sqlx::sqlite::SqliteRow) -> Session {
    use uhorse_core::types::IsolationLevel;

    let id: String = row.get("id");
    let channel: String = row.get("channel");
    let channel_user_id: String = row.get("channel_user_id");
    let created_at: i64 = row.get("created_at");
    let updated_at: i64 = row.get("updated_at");
    let metadata: String = row.get("metadata");
    let isolation_level: i32 = row.get("isolation_level");

    let channel = string_to_channel(&channel).unwrap_or(ChannelType::Telegram);
    let metadata = serde_json::from_str(&metadata).unwrap_or_default();
    let isolation_level = match isolation_level {
        0 => IsolationLevel::None,
        1 => IsolationLevel::Channel,
        2 => IsolationLevel::User,
        _ => IsolationLevel::Full,
    };

    Session {
        id: SessionId(id),
        channel,
        channel_user_id,
        created_at: created_at as u64,
        updated_at: updated_at as u64,
        metadata,
        isolation_level,
    }
}

fn row_to_message(row: sqlx::sqlite::SqliteRow) -> Message {
    let id: String = row.get("id");
    let session_id: String = row.get("session_id");
    let role: String = row.get("role");
    let content: String = row.get("content");
    let timestamp: i64 = row.get("timestamp");
    let sequence: i64 = row.get("sequence");

    let role = string_to_role(&role);
    let content = serde_json::from_str(&content).unwrap_or(MessageContent::Text(String::new()));

    Message {
        id,
        session_id: SessionId(session_id),
        role,
        content,
        timestamp: timestamp as u64,
        sequence: sequence as u64,
    }
}
