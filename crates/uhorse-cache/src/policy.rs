//! Cache eviction policies

use std::time::Duration;

/// Cache eviction policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least Recently Used - evict least recently accessed items
    LRU,
    /// Least Frequently Used - evict least frequently accessed items
    LFU,
    /// First In First Out - evict oldest items
    FIFO,
    /// No eviction - return error when full
    NoEviction,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::LRU
    }
}

/// Cache policy configuration
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Maximum number of items in cache
    pub max_items: usize,
    /// Maximum memory size in bytes (0 = unlimited)
    pub max_memory_bytes: usize,
    /// Default TTL for cache entries
    pub default_ttl: Duration,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// Whether to refresh TTL on access
    pub refresh_ttl_on_access: bool,
    /// Sample size for LRU/LFU eviction (larger = more accurate but slower)
    pub eviction_sample_size: usize,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_items: 10_000,
            max_memory_bytes: 0,                   // Unlimited
            default_ttl: Duration::from_secs(300), // 5 minutes
            eviction_policy: EvictionPolicy::default(),
            refresh_ttl_on_access: true,
            eviction_sample_size: 10,
        }
    }
}

impl CachePolicy {
    /// Create a new cache policy with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of items
    pub fn with_max_items(mut self, max_items: usize) -> Self {
        self.max_items = max_items;
        self
    }

    /// Set maximum memory size
    pub fn with_max_memory(mut self, max_memory_bytes: usize) -> Self {
        self.max_memory_bytes = max_memory_bytes;
        self
    }

    /// Set default TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Set eviction policy
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// Set TTL refresh on access
    pub fn with_refresh_ttl(mut self, refresh: bool) -> Self {
        self.refresh_ttl_on_access = refresh;
        self
    }
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntryMeta {
    /// When the entry was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the entry expires
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of times accessed
    pub access_count: u64,
    /// Last access time
    pub last_accessed_at: chrono::DateTime<chrono::Utc>,
    /// Size in bytes (estimated)
    pub size_bytes: usize,
}

impl CacheEntryMeta {
    /// Create new entry metadata
    pub fn new(ttl: Option<Duration>) -> Self {
        let now = chrono::Utc::now();
        Self {
            created_at: now,
            expires_at: ttl.map(|d| now + chrono::Duration::from_std(d).unwrap()),
            access_count: 0,
            last_accessed_at: now,
            size_bytes: 0,
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| chrono::Utc::now() > exp)
            .unwrap_or(false)
    }

    /// Record an access
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed_at = chrono::Utc::now();
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Option<Duration> {
        self.expires_at.map(|exp| {
            (exp - chrono::Utc::now())
                .to_std()
                .unwrap_or(Duration::ZERO)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_policy_default() {
        let policy = CachePolicy::default();
        assert_eq!(policy.max_items, 10_000);
        assert_eq!(policy.eviction_policy, EvictionPolicy::LRU);
    }

    #[test]
    fn test_cache_policy_builder() {
        let policy = CachePolicy::new()
            .with_max_items(1000)
            .with_ttl(Duration::from_secs(60))
            .with_eviction_policy(EvictionPolicy::LFU);

        assert_eq!(policy.max_items, 1000);
        assert_eq!(policy.default_ttl, Duration::from_secs(60));
        assert_eq!(policy.eviction_policy, EvictionPolicy::LFU);
    }

    #[test]
    fn test_entry_expiration() {
        let meta = CacheEntryMeta::new(Some(Duration::from_millis(10)));
        assert!(!meta.is_expired());

        std::thread::sleep(Duration::from_millis(20));
        assert!(meta.is_expired());
    }

    #[test]
    fn test_entry_access_tracking() {
        let mut meta = CacheEntryMeta::new(None);
        assert_eq!(meta.access_count, 0);

        meta.record_access();
        meta.record_access();
        assert_eq!(meta.access_count, 2);
    }
}
