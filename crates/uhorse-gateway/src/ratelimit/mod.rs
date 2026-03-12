//! Rate Limiting Module
//!
//! API 限流模块，支持多种限流策略

mod global;
mod user;
mod endpoint;
mod distributed;

pub use global::{GlobalRateLimiter, GlobalRateLimitConfig};
pub use user::{UserRateLimiter, UserRateLimitConfig, RateLimitKey};
pub use endpoint::{EndpointRateLimiter, EndpointRateLimitConfig, EndpointLimit};
pub use distributed::{DistributedRateLimiter, DistributedConfig, RateLimitBackend};

/// 限流结果
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// 允许请求
    Allowed {
        /// 剩余配额
        remaining: u64,
        /// 重置时间 (秒)
        reset_after: u64,
    },
    /// 拒绝请求
    Denied {
        /// 等待时间 (秒)
        retry_after: u64,
        /// 限制值
        limit: u64,
    },
}

impl RateLimitResult {
    /// 是否允许
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    /// 获取剩余配额
    pub fn remaining(&self) -> Option<u64> {
        match self {
            Self::Allowed { remaining, .. } => Some(*remaining),
            _ => None,
        }
    }

    /// 获取重试等待时间
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::Denied { retry_after, .. } => Some(*retry_after),
            _ => None,
        }
    }
}

/// 限流算法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitAlgorithm {
    /// 固定窗口
    FixedWindow,
    /// 滑动窗口
    SlidingWindow,
    /// 令牌桶
    TokenBucket,
    /// 漏桶
    LeakyBucket,
}

impl Default for RateLimitAlgorithm {
    fn default() -> Self {
        Self::SlidingWindow
    }
}

/// 限流配置基类
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 算法类型
    pub algorithm: RateLimitAlgorithm,
    /// 时间窗口大小 (秒)
    pub window_size: u64,
    /// 窗口内最大请求数
    pub max_requests: u64,
    /// 是否启用
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            algorithm: RateLimitAlgorithm::default(),
            window_size: 60,
            max_requests: 100,
            enabled: true,
        }
    }
}
