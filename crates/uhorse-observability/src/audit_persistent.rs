//! Audit log persistence and integrity
//!
//! 审计日志持久化与完整性保护

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::{AuditCategory, AuditEvent, AuditLevel};

/// 审计存储错误
#[derive(Debug, thiserror::Error)]
pub enum AuditStorageError {
    /// 存储错误
    #[error("Storage error: {0}")]
    StorageError(String),
    /// 查询错误
    #[error("Query error: {0}")]
    QueryError(String),
    /// 签名验证失败
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    /// 链完整性验证失败
    #[error("Chain integrity verification failed at index {index}")]
    ChainIntegrityFailed { index: usize },
    /// 未找到
    #[error("Not found: {0}")]
    NotFound(String),
}

/// 审计存储结果类型
pub type AuditResult<T> = std::result::Result<T, AuditStorageError>;

/// 签名后的审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAuditEvent {
    /// 原始事件
    pub event: AuditEvent,
    /// 事件哈希
    pub hash: String,
    /// 前一事件哈希 (用于链式验证)
    pub prev_hash: Option<String>,
    /// 签名时间戳
    pub signed_at: i64,
    /// 序列号
    pub sequence: u64,
}

impl SignedAuditEvent {
    /// 创建新的签名事件
    pub fn new(event: AuditEvent, prev_hash: Option<String>, sequence: u64) -> Self {
        let signed_at = Utc::now().timestamp();
        let hash = Self::compute_hash(&event, &prev_hash, signed_at, sequence);

        Self {
            event,
            hash,
            prev_hash,
            signed_at,
            sequence,
        }
    }

    /// 计算事件哈希
    fn compute_hash(
        event: &AuditEvent,
        prev_hash: &Option<String>,
        signed_at: i64,
        sequence: u64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(event.timestamp.to_le_bytes());
        hasher.update(format!("{:?}", event.level).as_bytes());
        hasher.update(format!("{:?}", event.category).as_bytes());
        hasher.update(event.action.as_bytes());

        if let Some(actor) = &event.actor {
            hasher.update(actor.as_bytes());
        }
        if let Some(target) = &event.target {
            hasher.update(target.as_bytes());
        }
        if let Some(session_id) = &event.session_id {
            hasher.update(session_id.as_bytes());
        }
        if let Some(prev) = prev_hash {
            hasher.update(prev.as_bytes());
        }
        hasher.update(signed_at.to_le_bytes());
        hasher.update(sequence.to_le_bytes());

        format!("{:x}", hasher.finalize())
    }

    /// 验证事件完整性
    pub fn verify(&self) -> bool {
        let computed_hash =
            Self::compute_hash(&self.event, &self.prev_hash, self.signed_at, self.sequence);
        computed_hash == self.hash
    }
}

/// 审计存储后端 trait
#[async_trait]
pub trait AuditStorage: Send + Sync {
    /// 存储审计事件
    async fn store(&self, event: &SignedAuditEvent) -> AuditResult<()>;

    /// 查询审计事件
    async fn query(&self, filter: &AuditQueryFilter) -> AuditResult<Vec<SignedAuditEvent>>;

    /// 获取单个事件
    async fn get(&self, sequence: u64) -> AuditResult<Option<SignedAuditEvent>>;

    /// 获取最新事件
    async fn get_latest(&self) -> AuditResult<Option<SignedAuditEvent>>;

    /// 验证链完整性
    async fn verify_chain(&self, from_seq: u64, to_seq: u64) -> AuditResult<()>;

    /// 获取统计信息
    async fn stats(&self) -> AuditStorageStats;
}

/// 审计查询过滤器
#[derive(Debug, Clone, Default)]
pub struct AuditQueryFilter {
    /// 开始时间
    pub start_time: Option<i64>,
    /// 结束时间
    pub end_time: Option<i64>,
    /// 事件级别
    pub level: Option<AuditLevel>,
    /// 事件类别
    pub category: Option<AuditCategory>,
    /// 操作者
    pub actor: Option<String>,
    /// 目标
    pub target: Option<String>,
    /// 会话 ID
    pub session_id: Option<String>,
    /// 限制数量
    pub limit: Option<usize>,
    /// 偏移量
    pub offset: Option<usize>,
}

/// 审计存储统计
#[derive(Debug, Clone, Default)]
pub struct AuditStorageStats {
    /// 总事件数
    pub total_events: u64,
    /// 最新序列号
    pub latest_sequence: u64,
    /// 最早时间戳
    pub earliest_timestamp: Option<i64>,
    /// 最新时间戳
    pub latest_timestamp: Option<i64>,
    /// 按类别统计
    pub by_category: std::collections::HashMap<String, u64>,
    /// 按级别统计
    pub by_level: std::collections::HashMap<String, u64>,
}

/// 内存审计存储 (用于测试和开发)
pub struct InMemoryAuditStorage {
    events: Arc<RwLock<VecDeque<SignedAuditEvent>>>,
    max_events: usize,
}

impl InMemoryAuditStorage {
    /// 创建新的内存存储
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(max_events))),
            max_events,
        }
    }
}

impl Default for InMemoryAuditStorage {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[async_trait]
impl AuditStorage for InMemoryAuditStorage {
    async fn store(&self, event: &SignedAuditEvent) -> AuditResult<()> {
        let mut events = self.events.write().await;

        // 容量管理
        while events.len() >= self.max_events {
            events.pop_front();
            warn!("Audit log overflow, dropping oldest event");
        }

        events.push_back(event.clone());
        debug!("Stored audit event seq={}", event.sequence);
        Ok(())
    }

    async fn query(&self, filter: &AuditQueryFilter) -> AuditResult<Vec<SignedAuditEvent>> {
        let events = self.events.read().await;
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let filtered: Vec<_> = events
            .iter()
            .skip(offset)
            .filter(|e| {
                // 时间范围过滤
                if let Some(start) = filter.start_time {
                    if e.event.timestamp < start as u64 {
                        return false;
                    }
                }
                if let Some(end) = filter.end_time {
                    if e.event.timestamp > end as u64 {
                        return false;
                    }
                }

                // 级别过滤
                if let Some(ref level) = filter.level {
                    if e.event.level != *level {
                        return false;
                    }
                }

                // 类别过滤
                if let Some(ref category) = filter.category {
                    if e.event.category != *category {
                        return false;
                    }
                }

                // Actor 过滤
                if let Some(ref actor) = filter.actor {
                    if e.event.actor.as_ref() != Some(actor) {
                        return false;
                    }
                }

                // Target 过滤
                if let Some(ref target) = filter.target {
                    if e.event.target.as_ref() != Some(target) {
                        return false;
                    }
                }

                // Session ID 过滤
                if let Some(ref session_id) = filter.session_id {
                    if e.event.session_id.as_ref() != Some(session_id) {
                        return false;
                    }
                }

                true
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn get(&self, sequence: u64) -> AuditResult<Option<SignedAuditEvent>> {
        let events = self.events.read().await;
        Ok(events.iter().find(|e| e.sequence == sequence).cloned())
    }

    async fn get_latest(&self) -> AuditResult<Option<SignedAuditEvent>> {
        let events = self.events.read().await;
        Ok(events.back().cloned())
    }

    async fn verify_chain(&self, from_seq: u64, to_seq: u64) -> AuditResult<()> {
        let events = self.events.read().await;
        let relevant: Vec<_> = events
            .iter()
            .filter(|e| e.sequence >= from_seq && e.sequence <= to_seq)
            .collect();

        if relevant.is_empty() {
            return Ok(());
        }

        // 验证每个事件自身的完整性
        for event in &relevant {
            if !event.verify() {
                return Err(AuditStorageError::SignatureVerificationFailed);
            }
        }

        // 验证链式哈希
        for i in 1..relevant.len() {
            let prev = &relevant[i - 1];
            let curr = &relevant[i];

            if curr.prev_hash.as_ref() != Some(&prev.hash) {
                return Err(AuditStorageError::ChainIntegrityFailed {
                    index: curr.sequence as usize,
                });
            }
        }

        info!(
            "Audit chain verified: {} events from {} to {}",
            relevant.len(),
            from_seq,
            to_seq
        );
        Ok(())
    }

    async fn stats(&self) -> AuditStorageStats {
        let events = self.events.read().await;
        let mut stats = AuditStorageStats {
            total_events: events.len() as u64,
            ..AuditStorageStats::default()
        };

        if let Some(first) = events.front() {
            stats.earliest_timestamp = Some(first.signed_at);
            stats.latest_sequence = first.sequence;
        }

        if let Some(last) = events.back() {
            stats.latest_timestamp = Some(last.signed_at);
            stats.latest_sequence = last.sequence;
        }

        for event in events.iter() {
            let category = format!("{:?}", event.event.category);
            let level = format!("{:?}", event.event.level);
            *stats.by_category.entry(category).or_insert(0) += 1;
            *stats.by_level.entry(level).or_insert(0) += 1;
        }

        stats
    }
}

/// 审计日志持久化器
pub struct AuditPersistor<S: AuditStorage> {
    storage: Arc<S>,
    sequence_counter: Arc<RwLock<u64>>,
}

impl<S: AuditStorage + 'static> AuditPersistor<S> {
    /// 创建新的持久化器
    pub fn new(storage: Arc<S>) -> Self {
        Self {
            storage,
            sequence_counter: Arc::new(RwLock::new(1)),
        }
    }

    /// 持久化审计事件
    pub async fn persist(&self, event: AuditEvent) -> AuditResult<SignedAuditEvent> {
        // 获取序列号
        let (sequence, prev_hash) = {
            let mut counter = self.sequence_counter.write().await;
            let seq = *counter;
            *counter += 1;

            // 获取前一事件哈希
            let prev = self.storage.get_latest().await?;
            let prev_hash = prev.map(|p| p.hash);

            (seq, prev_hash)
        };

        // 创建签名事件
        let signed = SignedAuditEvent::new(event, prev_hash, sequence);

        // 存储事件
        self.storage.store(&signed).await?;

        info!(
            "Persisted audit event seq={} hash={}",
            sequence,
            &signed.hash[..16]
        );
        Ok(signed)
    }

    /// 批量导出审计日志
    pub async fn export(&self, filter: &AuditQueryFilter) -> AuditResult<Vec<SignedAuditEvent>> {
        self.storage.query(filter).await
    }

    /// 验证审计日志完整性
    pub async fn verify(&self, from_seq: Option<u64>, to_seq: Option<u64>) -> AuditResult<()> {
        let from = from_seq.unwrap_or(1);
        let to = match to_seq {
            Some(t) => t,
            None => {
                let stats = self.storage.stats().await;
                stats.latest_sequence
            }
        };

        self.storage.verify_chain(from, to).await
    }

    /// 获取存储统计
    pub async fn stats(&self) -> AuditStorageStats {
        self.storage.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_event_verification() {
        let event = AuditEvent {
            timestamp: 1234567890,
            level: AuditLevel::Info,
            category: AuditCategory::Auth,
            actor: Some("user-1".to_string()),
            action: "login".to_string(),
            target: None,
            details: None,
            session_id: None,
        };

        let signed = SignedAuditEvent::new(event, None, 1);
        assert!(signed.verify());
    }

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = Arc::new(InMemoryAuditStorage::new(100));
        let persistor = AuditPersistor::new(storage.clone());

        let event = AuditEvent {
            timestamp: 1234567890,
            level: AuditLevel::Info,
            category: AuditCategory::Auth,
            actor: Some("user-1".to_string()),
            action: "login".to_string(),
            target: None,
            details: None,
            session_id: None,
        };

        let signed = persistor.persist(event).await.unwrap();
        assert!(signed.verify());
    }

    #[tokio::test]
    async fn test_chain_verification() {
        let storage = Arc::new(InMemoryAuditStorage::new(100));
        let persistor = AuditPersistor::new(storage.clone());

        // 添加多个事件
        for i in 0..5 {
            let event = AuditEvent {
                timestamp: 1234567890 + i,
                level: AuditLevel::Info,
                category: AuditCategory::Auth,
                actor: Some(format!("user-{}", i)),
                action: "action".to_string(),
                target: None,
                details: None,
                session_id: None,
            };
            persistor.persist(event).await.unwrap();
        }

        // 验证链完整性
        persistor.verify(None, None).await.unwrap();
    }
}
