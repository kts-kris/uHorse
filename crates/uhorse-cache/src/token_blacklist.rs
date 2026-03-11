//! Token blacklist implementation for revoked tokens

use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::policy::CachePolicy;
use super::redis::RedisCache;

/// Token information stored in blacklist
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlacklistedToken {
    /// Token ID (jti claim)
    pub token_id: String,
    /// User/tenant ID
    pub tenant_id: String,
    /// Reason for blacklisting
    pub reason: String,
    /// When the token was blacklisted
    pub blacklisted_at: i64,
    /// Token expiration time
    pub expires_at: i64,
}

impl BlacklistedToken {
    /// Create a new blacklisted token entry
    pub fn new(token_id: impl Into<String>, tenant_id: impl Into<String>, reason: impl Into<String>, expires_in: Duration) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            token_id: token_id.into(),
            tenant_id: tenant_id.into(),
            reason: reason.into(),
            blacklisted_at: now,
            expires_at: now + expires_in.as_secs() as i64,
        }
    }

    /// Check if the blacklist entry has expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.expires_at
    }

    /// Serialize to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from JSON bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// Token blacklist with L1 (local) and L2 (Redis) layers
pub struct TokenBlacklist {
    /// Local hash set (L1)
    local: Arc<RwLock<HashSet<String>>>,
    /// Redis cache (L2) - optional
    redis: Option<RedisCache>,
    /// Cache policy
    policy: CachePolicy,
    /// Default TTL for blacklisted tokens
    default_ttl: Duration,
}

impl TokenBlacklist {
    /// Create a new token blacklist with local cache only
    pub fn new_local(policy: CachePolicy) -> Self {
        Self {
            local: Arc::new(RwLock::new(HashSet::new())),
            redis: None,
            policy,
            default_ttl: Duration::from_secs(86400), // 24 hours default
        }
    }

    /// Create a new token blacklist with Redis backend
    pub fn new_with_redis(redis: RedisCache, policy: CachePolicy) -> Self {
        Self {
            local: Arc::new(RwLock::new(HashSet::new())),
            redis: Some(redis),
            policy,
            default_ttl: Duration::from_secs(86400),
        }
    }

    /// Set default TTL
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Add a token to the blacklist
    pub async fn add(&self, token: BlacklistedToken) -> Result<()> {
        let token_id = token.token_id.clone();
        let ttl = Duration::from_secs((token.expires_at - chrono::Utc::now().timestamp()).max(0) as u64);

        // Add to local cache
        {
            let mut local = self.local.write().await;
            local.insert(token_id.clone());
        }

        // Add to Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("blacklist:{}", token_id);
            let data = token.to_bytes()?;
            let effective_ttl = if ttl.is_zero() { self.default_ttl } else { ttl };
            redis.set_with_ttl(&key, &data, effective_ttl).await?;
            debug!("Token {} added to Redis blacklist", token_id);
        }

        info!("Token {} blacklisted", token_id);
        Ok(())
    }

    /// Check if a token is blacklisted
    pub async fn is_blacklisted(&self, token_id: &str) -> Result<bool> {
        // Check local cache first
        {
            let local = self.local.read().await;
            if local.contains(token_id) {
                debug!("Token {} found in local blacklist", token_id);
                return Ok(true);
            }
        }

        // Check Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("blacklist:{}", token_id);
            if redis.exists(&key).await? {
                // Populate local cache
                {
                    let mut local = self.local.write().await;
                    local.insert(token_id.to_string());
                }
                debug!("Token {} found in Redis blacklist", token_id);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Remove a token from the blacklist (e.g., after natural expiration)
    pub async fn remove(&self, token_id: &str) -> Result<bool> {
        let mut removed = false;

        // Remove from local cache
        {
            let mut local = self.local.write().await;
            removed = local.remove(token_id) || removed;
        }

        // Remove from Redis if available
        if let Some(ref redis) = self.redis {
            let key = format!("blacklist:{}", token_id);
            if redis.delete(&key).await? {
                removed = true;
                debug!("Token {} removed from Redis blacklist", token_id);
            }
        }

        if removed {
            info!("Token {} removed from blacklist", token_id);
        }
        Ok(removed)
    }

       /// Get blacklisted token details
    pub async fn get(&self, token_id: &str) -> Result<Option<BlacklistedToken>> {
        // Only check Redis for full details
        if let Some(ref _redis) = self.redis {
            let key = format!("blacklist:{}", token_id);
            if let Some(data) = _redis.get(&key).await? {
                let token = BlacklistedToken::from_bytes(&data)?;
                return Ok(Some(token));
            }
        }

        Ok(None)
    }

    /// Clean up expired entries from local cache
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let mut count = 0u64;

        // Note: Local cache doesn't track expiration, so we rely on Redis TTL
        // For a more complete solution, you'd track expiration times locally

        if let Some(ref redis) = self.redis {
            // Redis handles TTL automatically
            debug!("Redis TTL handles blacklist expiration automatically");
        }

        Ok(count)
    }

    /// Get blacklist statistics
    pub async fn stats(&self) -> BlacklistStats {
        let local = self.local.read().await;
        BlacklistStats {
            local_count: local.len(),
            max_items: self.policy.max_items,
        }
    }

    /// Clear all entries (use with caution)
    pub async fn clear(&self) -> Result<u64> {
        let mut count = 0u64;

        // Clear local cache
        {
            let mut local = self.local.write().await;
            count = local.len() as u64;
            local.clear();
        }

        // Clear Redis if available
        if let Some(ref redis) = self.redis {
            count = redis.delete_pattern("blacklist:*").await?;
        }

        warn!("Blacklist cleared: {} entries removed", count);
        Ok(count)
    }
}

/// Blacklist statistics
#[derive(Debug, Clone)]
pub struct BlacklistStats {
    /// Number of entries in local cache
    pub local_count: usize,
    /// Maximum items
    pub max_items: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklisted_token() {
        let token = BlacklistedToken::new(
            "token-123",
            "tenant-001",
            "User logged out",
            Duration::from_secs(3600),
        );

        assert_eq!(token.token_id, "token-123");
        assert_eq!(token.tenant_id, "tenant-001");
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_expiration() {
        let token = BlacklistedToken::new(
            "token-123",
            "tenant-001",
            "Test",
            Duration::from_millis(10),
        );

        assert!(!token.is_expired());
        std::thread::sleep(Duration::from_millis(20));
        assert!(token.is_expired());
    }

    #[tokio::test]
    async fn test_local_blacklist() {
        let blacklist = TokenBlacklist::new_local(CachePolicy::default());
        let token = BlacklistedToken::new(
            "token-123",
            "tenant-001",
            "Test",
            Duration::from_secs(3600),
        );

        blacklist.add(token).await.unwrap();
        assert!(blacklist.is_blacklisted("token-123").await.unwrap());
    }

    #[tokio::test]
    async fn test_blacklist_removal() {
        let blacklist = TokenBlacklist::new_local(CachePolicy::default());
        let token = BlacklistedToken::new(
            "token-123",
            "tenant-001",
            "Test",
            Duration::from_secs(3600),
        );

        blacklist.add(token).await.unwrap();
        blacklist.remove("token-123").await.unwrap();
        assert!(!blacklist.is_blacklisted("token-123").await.unwrap());
    }
}
