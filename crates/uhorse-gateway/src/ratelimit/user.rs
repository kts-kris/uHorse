//! User Rate Limiter
//!
//! 用户/租户级别的限流

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::{RateLimitAlgorithm, RateLimitConfig, RateLimitResult};

/// 用户限流配置
#[derive(Debug, Clone)]
pub struct UserRateLimitConfig {
    /// 基础配置
    pub base: RateLimitConfig,
    /// 默认用户配额
    pub default_quota: u64,
    /// VIP 用户配额
    pub vip_quota: u64,
    /// 企业用户配额
    pub enterprise_quota: u64,
}

impl Default for UserRateLimitConfig {
    fn default() -> Self {
        Self {
            base: RateLimitConfig::default(),
            default_quota: 100,
            vip_quota: 1000,
            enterprise_quota: 10000,
        }
    }
}

/// 限流键
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct RateLimitKey {
    /// 租户 ID
    pub tenant_id: String,
    /// 用户 ID
    pub user_id: String,
}

impl RateLimitKey {
    /// 创建新的限流键
    pub fn new(tenant_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
        }
    }
}

/// 用户配额
#[derive(Debug, Clone)]
pub struct UserQuota {
    /// 用户类型
    pub user_type: UserType,
    /// 配额
    pub quota: u64,
    /// 当前使用量
    pub current_usage: u64,
    /// 窗口开始时间
    pub window_start: Instant,
}

/// 用户类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserType {
    /// 默认用户
    Default,
    /// VIP 用户
    Vip,
    /// 企业用户
    Enterprise,
}

impl UserType {
    /// 获取配额
    pub fn quota(&self, config: &UserRateLimitConfig) -> u64 {
        match self {
            Self::Default => config.default_quota,
            Self::Vip => config.vip_quota,
            Self::Enterprise => config.enterprise_quota,
        }
    }
}

/// 用户限流器
pub struct UserRateLimiter {
    /// 配置
    config: UserRateLimitConfig,
    /// 用户配额映射
    quotas: Arc<RwLock<HashMap<RateLimitKey, UserQuota>>>,
}

impl UserRateLimiter {
    /// 创建新的用户限流器
    pub fn new(config: UserRateLimitConfig) -> Self {
        Self {
            config,
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查是否允许请求
    pub async fn check(&self, key: &RateLimitKey) -> RateLimitResult {
        if !self.config.base.enabled {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
                reset_after: 0,
            };
        }

        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.base.window_size);

        let mut quotas = self.quotas.write().await;

        // 获取或创建配额
        let quota = quotas.entry(key.clone()).or_insert_with(|| UserQuota {
            user_type: UserType::Default,
            quota: self.config.default_quota,
            current_usage: 0,
            window_start: now,
        });

        // 检查是否需要重置窗口
        if now.duration_since(quota.window_start) >= window_duration {
            quota.window_start = now;
            quota.current_usage = 0;
        }

        // 检查配额
        if quota.current_usage >= quota.quota {
            let elapsed = now.duration_since(quota.window_start);
            let reset_after = window_duration.saturating_sub(elapsed).as_secs();

            return RateLimitResult::Denied {
                retry_after: reset_after.max(1),
                limit: quota.quota,
            };
        }

        // 增加使用量
        quota.current_usage += 1;
        let remaining = quota.quota - quota.current_usage;

        let elapsed = now.duration_since(quota.window_start);
        let reset_after = window_duration.saturating_sub(elapsed).as_secs();

        RateLimitResult::Allowed {
            remaining,
            reset_after,
        }
    }

    /// 设置用户类型
    pub async fn set_user_type(&self, key: &RateLimitKey, user_type: UserType) {
        let mut quotas = self.quotas.write().await;

        if let Some(quota) = quotas.get_mut(key) {
            quota.user_type = user_type;
            quota.quota = user_type.quota(&self.config);
        } else {
            quotas.insert(
                key.clone(),
                UserQuota {
                    user_type,
                    quota: user_type.quota(&self.config),
                    current_usage: 0,
                    window_start: Instant::now(),
                },
            );
        }
    }

    /// 获取用户状态
    pub async fn get_status(&self, key: &RateLimitKey) -> Option<UserRateLimitStatus> {
        let quotas = self.quotas.read().await;
        quotas.get(key).map(|q| UserRateLimitStatus {
            user_type: q.user_type,
            quota: q.quota,
            current_usage: q.current_usage,
            remaining: q.quota.saturating_sub(q.current_usage),
        })
    }

    /// 重置用户配额
    pub async fn reset(&self, key: &RateLimitKey) -> bool {
        let mut quotas = self.quotas.write().await;
        if let Some(quota) = quotas.get_mut(key) {
            quota.current_usage = 0;
            quota.window_start = Instant::now();
            true
        } else {
            false
        }
    }

    /// 清理过期条目
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.base.window_size);

        let mut quotas = self.quotas.write().await;
        quotas.retain(|_, quota| now.duration_since(quota.window_start) < window_duration * 2);
    }
}

impl Default for UserRateLimiter {
    fn default() -> Self {
        Self::new(UserRateLimitConfig::default())
    }
}

/// 用户限流状态
#[derive(Debug, Clone)]
pub struct UserRateLimitStatus {
    /// 用户类型
    pub user_type: UserType,
    /// 配额
    pub quota: u64,
    /// 当前使用量
    pub current_usage: u64,
    /// 剩余配额
    pub remaining: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_user_rate_limiter() {
        let config = UserRateLimitConfig {
            base: RateLimitConfig {
                max_requests: 10,
                ..Default::default()
            },
            default_quota: 5,
            ..Default::default()
        };

        let limiter = UserRateLimiter::new(config);
        let key = RateLimitKey::new("tenant-1", "user-1");

        // 前 5 个请求应该成功
        for _ in 0..5 {
            let result = limiter.check(&key).await;
            assert!(result.is_allowed());
        }

        // 第 6 个请求应该被拒绝
        let result = limiter.check(&key).await;
        assert!(!result.is_allowed());
    }

    #[tokio::test]
    async fn test_user_type_upgrade() {
        let config = UserRateLimitConfig {
            default_quota: 5,
            vip_quota: 10,
            ..Default::default()
        };

        let limiter = UserRateLimiter::new(config);
        let key = RateLimitKey::new("tenant-1", "user-1");

        // 设置为 VIP
        limiter.set_user_type(&key, UserType::Vip).await;

        // 应该有 10 个配额
        for _ in 0..10 {
            let result = limiter.check(&key).await;
            assert!(result.is_allowed());
        }

        // 第 11 个应该被拒绝
        let result = limiter.check(&key).await;
        assert!(!result.is_allowed());
    }
}
