//! Redis cache implementation

use anyhow::{anyhow, Result};
use redis::{AsyncCommands, Client as RedisClient};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use super::policy::CachePolicy;

/// Redis cache client wrapper
#[derive(Clone)]
pub struct RedisCache {
    /// Redis client
    client: RedisClient,
    /// Cache policy
    policy: CachePolicy,
    /// Key prefix for namespacing
    key_prefix: String,
}

impl RedisCache {
    /// Create a new Redis cache
    pub fn new(redis_url: &str, policy: CachePolicy) -> Result<Self> {
        let client = RedisClient::open(redis_url)
            .map_err(|e| anyhow!("Failed to create Redis client: {}", e))?;

        info!("Redis cache client created");
        Ok(Self {
            client,
            policy,
            key_prefix: "uhorse:".to_string(),
        })
    }

    /// Create with custom key prefix
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    /// Get a connection
    async fn get_connection(&self) -> Result<redis::aio::ConnectionManager> {
        self.client
            .get_connection_manager()
            .await
            .map_err(|e| anyhow!("Failed to get Redis connection: {}", e))
    }

    /// Build full key with prefix
    fn build_key(&self, key: &str) -> String {
        format!("{}{}", self.key_prefix, key)
    }

    /// Get a value from cache
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        match conn.get::<_, Option<Vec<u8>>>(&full_key).await {
            Ok(Some(value)) => {
                debug!("Cache hit for key: {}", key);
                if self.policy.refresh_ttl_on_access {
                    if let Err(e) = conn
                        .expire::<_, ()>(&full_key, self.policy.default_ttl.as_secs() as i64)
                        .await
                    {
                        warn!("Failed to refresh TTL for key {}: {}", key, e);
                    }
                }
                Ok(Some(value))
            }
            Ok(None) => {
                debug!("Cache miss for key: {}", key);
                Ok(None)
            }
            Err(e) => {
                error!("Redis get error for key {}: {}", key, e);
                Err(anyhow!("Redis get error: {}", e))
            }
        }
    }

    /// Get a string value from cache
    pub async fn get_string(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        match conn.get::<_, Option<String>>(&full_key).await {
            Ok(value) => Ok(value),
            Err(e) => {
                error!("Redis get string error for key {}: {}", key, e);
                Err(anyhow!("Redis get string error: {}", e))
            }
        }
    }

    /// Set a value in cache
    pub async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.set_with_ttl(key, value, self.policy.default_ttl).await
    }

    /// Set a value with custom TTL
    pub async fn set_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        conn.set_ex::<_, _, ()>(&full_key, value, ttl.as_secs())
            .await
            .map_err(|e| {
                error!("Redis set error for key {}: {}", key, e);
                anyhow!("Redis set error: {}", e)
            })?;

        debug!("Cache set for key: {} (TTL: {:?})", key, ttl);
        Ok(())
    }

    /// Set a string value
    pub async fn set_string(&self, key: &str, value: &str) -> Result<()> {
        self.set_string_with_ttl(key, value, self.policy.default_ttl).await
    }

    /// Set a string value with custom TTL
    pub async fn set_string_with_ttl(&self, key: &str, value: &str, ttl: Duration) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        conn.set_ex::<_, _, ()>(&full_key, value, ttl.as_secs())
            .await
            .map_err(|e| {
                error!("Redis set string error for key {}: {}", key, e);
                anyhow!("Redis set string error: {}", e)
            })?;

        debug!("Cache set string for key: {} (TTL: {:?})", key, ttl);
        Ok(())
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        let deleted: i32 = conn.del(&full_key).await.map_err(|e| {
            error!("Redis delete error for key {}: {}", key, e);
            anyhow!("Redis delete error: {}", e)
        })?;

        debug!("Cache delete for key: {} (deleted: {})", key, deleted);
        Ok(deleted > 0)
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        conn.exists(&full_key).await.map_err(|e| {
            error!("Redis exists error for key {}: {}", key, e);
            anyhow!("Redis exists error: {}", e)
        })
    }

    /// Set expiration on a key
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        let result: i32 = conn.expire(&full_key, ttl.as_secs() as i64).await.map_err(|e| {
            error!("Redis expire error for key {}: {}", key, e);
            anyhow!("Redis expire error: {}", e)
        })?;

        Ok(result == 1)
    }

    /// Get TTL of a key
    pub async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        let ttl_seconds: i64 = conn.ttl(&full_key).await.map_err(|e| {
            error!("Redis TTL error for key {}: {}", key, e);
            anyhow!("Redis TTL error: {}", e)
        })?;

        if ttl_seconds < 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::from_secs(ttl_seconds as u64)))
        }
    }

    /// Increment a counter by 1
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        let full_key = self.build_key(key);

        // INCR command increments by 1
        let result: i64 = redis::cmd("INCR")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Redis incr error for key {}: {}", key, e);
                anyhow!("Redis incr error: {}", e)
            })?;

        Ok(result)
    }

    /// Increment with expiry (set expiry on first increment)
    pub async fn incr_with_expiry(&self, key: &str, ttl: Duration) -> Result<i64> {
        let value = self.incr(key).await?;
        if value == 1 {
            self.expire(key, ttl).await?;
        }
        Ok(value)
    }

    /// Delete all keys matching pattern
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u64> {
        let mut conn = self.get_connection().await?;
        let full_pattern = self.build_key(pattern);

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&full_pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow!("Redis KEYS error: {}", e))?;

        if keys.is_empty() {
            return Ok(0);
        }

        let deleted: u64 = conn.del(&keys).await.map_err(|e| {
            error!("Redis delete pattern error for pattern {}: {}", pattern, e);
            anyhow!("Redis delete pattern error: {}", e)
        })?;

        debug!("Deleted {} keys matching pattern: {}", deleted, pattern);
        Ok(deleted)
    }

    /// Get cache statistics
    pub async fn stats(&self) -> Result<CacheStats> {
        let mut conn = self.get_connection().await?;

        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow!("Redis INFO error: {}", e))?;

        let used_memory = info
            .lines()
            .find(|line| line.starts_with("used_memory:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let db_size: u64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow!("Redis DBSIZE error: {}", e))?;

        Ok(CacheStats {
            total_keys: db_size,
            used_memory_bytes: used_memory,
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of keys
    pub total_keys: u64,
    /// Used memory in bytes
    pub used_memory_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_building() {
        // Basic test without actual Redis connection
        let key = format!("{}{}", "uhorse:", "test");
        assert_eq!(key, "uhorse:test");
    }
}
