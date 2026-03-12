//! Distributed Rate Limiter
//!
//! 基于 Redis 的分布式限流

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::{RateLimitAlgorithm, RateLimitConfig, RateLimitResult};

/// 分布式限流配置
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    /// 基础配置
    pub base: RateLimitConfig,
    /// Redis URL
    pub redis_url: String,
    /// 键前缀
    pub key_prefix: String,
    /// 连接超时 (毫秒)
    pub connection_timeout_ms: u64,
    /// 操作超时 (毫秒)
    pub operation_timeout_ms: u64,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            base: RateLimitConfig::default(),
            redis_url: "redis://127.0.0.1:6379".to_string(),
            key_prefix: "uhorse:ratelimit:".to_string(),
            connection_timeout_ms: 5000,
            operation_timeout_ms: 1000,
        }
    }
}

/// 限流后端接口
#[async_trait]
pub trait RateLimitBackend: Send + Sync {
    /// 增加计数并获取当前值
    async fn increment(&self, key: &str, window_secs: u64) -> Result<u64, BackendError>;

    /// 获取当前计数
    async fn get(&self, key: &str) -> Result<u64, BackendError>;

    /// 重置计数
    async fn reset(&self, key: &str) -> Result<(), BackendError>;

    /// 获取 TTL
    async fn ttl(&self, key: &str) -> Result<u64, BackendError>;

    /// 健康检查
    async fn health_check(&self) -> Result<bool, BackendError>;
}

/// 后端错误
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Operation timeout")]
    Timeout,

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// 内存后端 (用于测试)
pub struct MemoryBackend {
    counters: Arc<RwLock<std::collections::HashMap<String, (u64, std::time::Instant)>>>,
}

impl MemoryBackend {
    /// 创建新的内存后端
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RateLimitBackend for MemoryBackend {
    async fn increment(&self, key: &str, window_secs: u64) -> Result<u64, BackendError> {
        let now = std::time::Instant::now();
        let mut counters = self.counters.write().await;

        let (count, created_at) = counters.entry(key.to_string()).or_insert((0, now));

        // 检查是否过期
        if now.duration_since(*created_at) >= Duration::from_secs(window_secs) {
            *count = 0;
            *created_at = now;
        }

        *count += 1;
        Ok(*count)
    }

    async fn get(&self, key: &str) -> Result<u64, BackendError> {
        let counters = self.counters.read().await;
        Ok(counters.get(key).map(|(c, _)| *c).unwrap_or(0))
    }

    async fn reset(&self, key: &str) -> Result<(), BackendError> {
        let mut counters = self.counters.write().await;
        counters.remove(key);
        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<u64, BackendError> {
        let counters = self.counters.read().await;
        if let Some((_, created_at)) = counters.get(key) {
            let elapsed = std::time::Instant::now().duration_since(*created_at);
            Ok(elapsed.as_secs())
        } else {
            Ok(0)
        }
    }

    async fn health_check(&self) -> Result<bool, BackendError> {
        Ok(true)
    }
}

/// 分布式限流器
pub struct DistributedRateLimiter<B: RateLimitBackend> {
    /// 配置
    config: DistributedConfig,
    /// 后端
    backend: Arc<B>,
}

impl<B: RateLimitBackend> DistributedRateLimiter<B> {
    /// 创建新的分布式限流器
    pub fn new(config: DistributedConfig, backend: Arc<B>) -> Self {
        Self { config, backend }
    }

    /// 检查是否允许请求
    pub async fn check(&self, key: &str) -> RateLimitResult {
        if !self.config.base.enabled {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
                reset_after: 0,
            };
        }

        let full_key = format!("{}{}", self.config.key_prefix, key);

        // 增加计数
        let count = match self.backend.increment(&full_key, self.config.base.window_size).await {
            Ok(c) => c,
            Err(_) => {
                // 后端错误时降级为允许
                return RateLimitResult::Allowed {
                    remaining: u64::MAX,
                    reset_after: 0,
                };
            }
        };

        // 检查限制
        if count > self.config.base.max_requests {
            let ttl = self.backend.ttl(&full_key).await.unwrap_or(0);
            let retry_after = self.config.base.window_size.saturating_sub(ttl);

            return RateLimitResult::Denied {
                retry_after: retry_after.max(1),
                limit: self.config.base.max_requests,
            };
        }

        let remaining = self.config.base.max_requests.saturating_sub(count);
        let ttl = self.backend.ttl(&full_key).await.unwrap_or(0);
        let reset_after = self.config.base.window_size.saturating_sub(ttl);

        RateLimitResult::Allowed {
            remaining,
            reset_after,
        }
    }

    /// 重置限流器
    pub async fn reset(&self, key: &str) -> Result<(), BackendError> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        self.backend.reset(&full_key).await
    }

    /// 获取当前计数
    pub async fn get_count(&self, key: &str) -> Result<u64, BackendError> {
        let full_key = format!("{}{}", self.config.key_prefix, key);
        self.backend.get(&full_key).await
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, BackendError> {
        self.backend.health_check().await
    }

    /// 获取配置
    pub fn config(&self) -> &DistributedConfig {
        &self.config
    }
}

/// Redis 后端 (需要 redis crate)
#[cfg(feature = "redis")]
pub struct RedisBackend {
    client: redis::Client,
    connection: Arc<redis::aio::ConnectionManager>,
}

#[cfg(feature = "redis")]
impl RedisBackend {
    /// 创建新的 Redis 后端
    pub async fn new(url: &str) -> Result<Self, BackendError> {
        let client = redis::Client::open(url)
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        let connection = redis::aio::ConnectionManager::new(client.clone())
            .await
            .map_err(|e| BackendError::ConnectionError(e.to_string()))?;

        Ok(Self {
            client,
            connection: Arc::new(connection),
        })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl RateLimitBackend for RedisBackend {
    async fn increment(&self, key: &str, window_secs: u64) -> Result<u64, BackendError> {
        use redis::AsyncCommands;

        let mut conn = self.connection.clone();
        let count: u64 = redis::cmd("INCR")
            .arg(key)
            .query_async(&mut *conn)
            .await
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        // 如果是新键，设置过期时间
        if count == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(key)
                .arg(window_secs)
                .query_async(&mut *conn)
                .await
                .map_err(|e| BackendError::RedisError(e.to_string()))?;
        }

        Ok(count)
    }

    async fn get(&self, key: &str) -> Result<u64, BackendError> {
        use redis::AsyncCommands;

        let mut conn = self.connection.clone();
        let count: u64 = conn.get(key)
            .await
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        Ok(count)
    }

    async fn reset(&self, key: &str) -> Result<(), BackendError> {
        use redis::AsyncCommands;

        let mut conn = self.connection.clone();
        let _: () = conn.del(key)
            .await
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        Ok(())
    }

    async fn ttl(&self, key: &str) -> Result<u64, BackendError> {
        use redis::AsyncCommands;

        let mut conn = self.connection.clone();
        let ttl: i64 = conn.ttl(key)
            .await
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        Ok(ttl.max(0) as u64)
    }

    async fn health_check(&self) -> Result<bool, BackendError> {
        use redis::AsyncCommands;

        let mut conn = self.connection.clone();
        let pong: String = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| BackendError::RedisError(e.to_string()))?;

        Ok(pong == "PONG")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend() {
        let backend = MemoryBackend::new();

        // 增加计数
        let count = backend.increment("test:key", 60).await.unwrap();
        assert_eq!(count, 1);

        let count = backend.increment("test:key", 60).await.unwrap();
        assert_eq!(count, 2);

        // 获取计数
        let count = backend.get("test:key").await.unwrap();
        assert_eq!(count, 2);

        // 重置
        backend.reset("test:key").await.unwrap();
        let count = backend.get("test:key").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_distributed_rate_limiter() {
        let config = DistributedConfig {
            base: RateLimitConfig {
                max_requests: 5,
                ..Default::default()
            },
            ..Default::default()
        };

        let backend = Arc::new(MemoryBackend::new());
        let limiter = DistributedRateLimiter::new(config, backend);

        // 前 5 个请求应该成功
        for _ in 0..5 {
            let result = limiter.check("user:123").await;
            assert!(result.is_allowed());
        }

        // 第 6 个请求应该被拒绝
        let result = limiter.check("user:123").await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_distributed_rate_limiter_reset() {
        let config = DistributedConfig {
            base: RateLimitConfig {
                max_requests: 5,
                ..Default::default()
            },
            ..Default::default()
        };

        let backend = Arc::new(MemoryBackend::new());
        let limiter = DistributedRateLimiter::new(config, backend);

        // 用完配额
        for _ in 0..5 {
            limiter.check("user:123").await;
        }

        // 应该被拒绝
        let result = limiter.check("user:123").await;
        assert!(!result.is_allowed());

        // 重置
        limiter.reset("user:123").await.unwrap();

        // 应该可以再次请求
        let result = limiter.check("user:123").await;
        assert!(result.is_allowed());
    }
}
