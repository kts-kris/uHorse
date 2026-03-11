//! Session cache implementation

use anyhow::Result;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::policy::CachePolicy;
use super::redis::RedisCache;

/// Session data stored in cache
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionData {
    /// Session ID
    pub session_id: String,
    /// User/tenant ID
    pub tenant_id: String,
    /// Session metadata
    pub metadata: serde_json::Value,
    /// Created timestamp
    pub created_at: i64,
    /// Last accessed timestamp
    pub last_accessed_at: i64,
}

impl SessionData {
    /// Create new session data
    pub fn new(session_id: impl Into<String>, tenant_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            session_id: session_id.into(),
            tenant_id: tenant_id.into(),
            metadata: serde_json::json!({}),
            created_at: now,
            last_accessed_at: now,
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Touch the session (update last accessed)
    pub fn touch(&mut self) {
        self.last_accessed_at = chrono::Utc::now().timestamp();
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// Session cache with L1 (local) and L2 (Redis) layers
pub struct SessionCache {
    /// Local LRU cache (L1)
    local: Arc<RwLock<LruCache<String, SessionData>>>,
    /// Redis cache (L2) - optional
    redis: Option<RedisCache>,
    /// Cache policy
    policy: CachePolicy,
    /// Session TTL
    session_ttl: Duration,
}

impl SessionCache {
    /// Create a new session cache with local cache only
    pub fn new_local(policy: CachePolicy) -> Self {
        let max_items = NonZeroUsize::new(policy.max_items).unwrap_or(NonZeroUsize::new(1000).unwrap());

        Self {
            local: Arc::new(RwLock::new(LruCache::new(max_items))),
            redis: None,
            policy,
            session_ttl: Duration::from_secs(3600), // 1 hour default
        }
    }

    /// Create a new session cache with Redis backend
    pub fn new_with_redis(redis: RedisCache, policy: CachePolicy) -> Self {
        let max_items = NonZeroUsize::new(policy.max_items).unwrap_or(NonZeroUsize::new(1000).unwrap());

        Self {
            local: Arc::new(RwLock::new(LruCache::new(max_items))),
            redis: Some(redis),
            policy,
            session_ttl: Duration::from_secs(3600),
        }
    }

    /// Set session TTL
    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.session_ttl = ttl;
        self
    }

    /// Create or update a session
    pub async fn set(&self, session: SessionData) -> Result<()> {
        // Update local cache
        {
            let mut local = self.local.write().await;
            local.put(session.session_id.clone(), session.clone());
        }

        // Update Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("session:{}", session.session_id);
            let data = session.to_bytes()?;
            redis.set_with_ttl(&key, &data, self.session_ttl).await?;
            debug!("Session {} stored in Redis", session.session_id);
        }

        Ok(())
    }

    /// Get a session by ID
    pub async fn get(&self, session_id: &str) -> Result<Option<SessionData>> {
        // Check local cache first
        {
            let mut local = self.local.write().await;
            if let Some(session) = local.get_mut(session_id) {
                session.touch();
                debug!("Session {} found in local cache", session_id);
                return Ok(Some(session.clone()));
            }
        }

        // Check Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("session:{}", session_id);
            if let Some(data) = redis.get(&key).await? {
                let mut session = SessionData::from_bytes(&data)?;
                session.touch();

                // Populate local cache
                {
                    let mut local = self.local.write().await;
                    local.put(session_id.to_string(), session.clone());
                }

                debug!("Session {} loaded from Redis", session_id);
                return Ok(Some(session));
            }
        }

        debug!("Session {} not found", session_id);
        Ok(None)
    }

    /// Delete a session
    pub async fn delete(&self, session_id: &str) -> Result<bool> {
        // Remove from local cache
        {
            let mut local = self.local.write().await;
            local.pop(session_id);
        }

        // Remove from Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("session:{}", session_id);
            redis.delete(&key).await?;
            debug!("Session {} deleted from Redis", session_id);
        }

        Ok(true)
    }

    /// Check if session exists
    pub async fn exists(&self, session_id: &str) -> Result<bool> {
        // Check local cache
        {
            let local = self.local.read().await;
            if local.contains(session_id) {
                return Ok(true);
            }
        }

        // Check Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("session:{}", session_id);
            return redis.exists(&key).await;
        }

        Ok(false)
    }

    /// Refresh session TTL
    pub async fn refresh(&self, session_id: &str) -> Result<bool> {
        // Touch in local cache
        {
            let mut local = self.local.write().await;
            if let Some(session) = local.get_mut(session_id) {
                session.touch();
            }
        }

        // Refresh in Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("session:{}", session_id);
            return redis.expire(&key, self.session_ttl).await;
        }

        Ok(true)
    }

    /// Get all session IDs for a tenant
    pub async fn get_tenant_sessions(&self, tenant_id: &str) -> Result<Vec<String>> {
        let mut sessions = Vec::new();

        // Check local cache
        {
            let local = self.local.read().await;
            for (_, session) in local.iter() {
                if session.tenant_id == tenant_id {
                    sessions.push(session.session_id.clone());
                }
            }
        }

        // Note: Redis pattern scan would be expensive, so we rely on local cache
        // In production, you might want to maintain a tenant->sessions index

        Ok(sessions)
    }

    /// Delete all sessions for a tenant
    pub async fn delete_tenant_sessions(&self, tenant_id: &str) -> Result<u64> {
        let session_ids = self.get_tenant_sessions(tenant_id).await?;
        let count = session_ids.len() as u64;

        for session_id in session_ids {
            self.delete(&session_id).await?;
        }

        info!("Deleted {} sessions for tenant {}", count, tenant_id);
        Ok(count)
    }

    /// Get cache statistics
    pub async fn stats(&self) -> SessionCacheStats {
        let local = self.local.read().await;
        SessionCacheStats {
            local_cache_size: local.len(),
            max_items: self.policy.max_items,
        }
    }
}

/// Session cache statistics
#[derive(Debug, Clone)]
pub struct SessionCacheStats {
    /// Current local cache size
    pub local_cache_size: usize,
    /// Maximum items in local cache
    pub max_items: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_data() {
        let session = SessionData::new("sess-123", "tenant-001")
            .with_metadata(serde_json::json!({ "user": "test" }));

        assert_eq!(session.session_id, "sess-123");
        assert_eq!(session.tenant_id, "tenant-001");
    }

    #[test]
    fn test_session_serialization() {
        let session = SessionData::new("sess-123", "tenant-001");
        let bytes = session.to_bytes().unwrap();
        let decoded = SessionData::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.session_id, session.session_id);
    }

    #[tokio::test]
    async fn test_local_session_cache() {
        let cache = SessionCache::new_local(CachePolicy::default().with_max_items(100));
        let session = SessionData::new("sess-123", "tenant-001");

        cache.set(session.clone()).await.unwrap();
        let retrieved = cache.get("sess-123").await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().session_id, "sess-123");
    }

    #[tokio::test]
    async fn test_session_not_found() {
        let cache = SessionCache::new_local(CachePolicy::default());
        let result = cache.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_session_delete() {
        let cache = SessionCache::new_local(CachePolicy::default());
        let session = SessionData::new("sess-123", "tenant-001");

        cache.set(session).await.unwrap();
        cache.delete("sess-123").await.unwrap();

        let result = cache.get("sess-123").await.unwrap();
        assert!(result.is_none());
    }
}
