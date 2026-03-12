//! Global Rate Limiter
//!
//! 系统级别的全局限流

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{RateLimitAlgorithm, RateLimitConfig, RateLimitResult};

/// 全局限流配置
#[derive(Debug, Clone)]
pub struct GlobalRateLimitConfig {
    /// 基础配置
    pub base: RateLimitConfig,
    /// 最大并发连接数
    pub max_concurrent: u64,
    /// 每秒最大请求数
    pub requests_per_second: u64,
}

impl Default for GlobalRateLimitConfig {
    fn default() -> Self {
        Self {
            base: RateLimitConfig::default(),
            max_concurrent: 10000,
            requests_per_second: 1000,
        }
    }
}

/// 全局限流器
pub struct GlobalRateLimiter {
    /// 配置
    config: GlobalRateLimitConfig,
    /// 当前并发连接数
    current_concurrent: AtomicU64,
    /// 窗口内请求数
    window_requests: AtomicU64,
    /// 窗口开始时间
    window_start: std::sync::Mutex<Instant>,
}

impl GlobalRateLimiter {
    /// 创建新的全局限流器
    pub fn new(config: GlobalRateLimitConfig) -> Self {
        Self {
            config,
            current_concurrent: AtomicU64::new(0),
            window_requests: AtomicU64::new(0),
            window_start: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// 检查是否允许请求
    pub fn check(&self) -> RateLimitResult {
        if !self.config.base.enabled {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
                reset_after: 0,
            };
        }

        // 检查并发连接数
        let concurrent = self.current_concurrent.load(Ordering::Relaxed);
        if concurrent >= self.config.max_concurrent {
            return RateLimitResult::Denied {
                retry_after: 1,
                limit: self.config.max_concurrent,
            };
        }

        // 检查窗口内请求数
        self.check_window()
    }

    /// 检查时间窗口
    fn check_window(&self) -> RateLimitResult {
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.base.window_size);

        // 检查是否需要重置窗口
        {
            let mut start = self.window_start.lock().unwrap();
            if now.duration_since(*start) >= window_duration {
                *start = now;
                self.window_requests.store(0, Ordering::Relaxed);
            }
        }

        let requests = self.window_requests.load(Ordering::Relaxed);
        if requests >= self.config.base.max_requests {
            let start = self.window_start.lock().unwrap();
            let elapsed = now.duration_since(*start);
            let reset_after = window_duration.saturating_sub(elapsed).as_secs();

            return RateLimitResult::Denied {
                retry_after: reset_after.max(1),
                limit: self.config.base.max_requests,
            };
        }

        // 增加计数
        let remaining = self.config.base.max_requests - requests - 1;
        self.window_requests.fetch_add(1, Ordering::Relaxed);

        let start = self.window_start.lock().unwrap();
        let elapsed = now.duration_since(*start);
        let reset_after = window_duration.saturating_sub(elapsed).as_secs();

        RateLimitResult::Allowed {
            remaining,
            reset_after,
        }
    }

    /// 开始连接
    pub fn start_connection(&self) -> bool {
        let current = self.current_concurrent.fetch_add(1, Ordering::Relaxed);
        if current >= self.config.max_concurrent {
            self.current_concurrent.fetch_sub(1, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    /// 结束连接
    pub fn end_connection(&self) {
        self.current_concurrent.fetch_sub(1, Ordering::Relaxed);
    }

    /// 获取当前状态
    pub fn status(&self) -> GlobalRateLimitStatus {
        let concurrent = self.current_concurrent.load(Ordering::Relaxed);
        let requests = self.window_requests.load(Ordering::Relaxed);

        GlobalRateLimitStatus {
            current_concurrent: concurrent,
            max_concurrent: self.config.max_concurrent,
            current_requests: requests,
            max_requests: self.config.base.max_requests,
        }
    }
}

impl Default for GlobalRateLimiter {
    fn default() -> Self {
        Self::new(GlobalRateLimitConfig::default())
    }
}

/// 全局限流状态
#[derive(Debug, Clone)]
pub struct GlobalRateLimitStatus {
    /// 当前并发连接数
    pub current_concurrent: u64,
    /// 最大并发连接数
    pub max_concurrent: u64,
    /// 当前窗口请求数
    pub current_requests: u64,
    /// 最大请求数
    pub max_requests: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_rate_limiter() {
        let config = GlobalRateLimitConfig {
            base: RateLimitConfig {
                max_requests: 10,
                ..Default::default()
            },
            ..Default::default()
        };

        let limiter = GlobalRateLimiter::new(config);

        // 前 10 个请求应该成功
        for _ in 0..10 {
            let result = limiter.check();
            assert!(result.is_allowed());
        }

        // 第 11 个请求应该被拒绝
        let result = limiter.check();
        assert!(!result.is_allowed());
    }

    #[test]
    fn test_global_rate_limiter_concurrent() {
        let config = GlobalRateLimitConfig {
            max_concurrent: 5,
            ..Default::default()
        };

        let limiter = GlobalRateLimiter::new(config);

        // 开始 5 个连接
        for _ in 0..5 {
            assert!(limiter.start_connection());
        }

        // 第 6 个应该失败
        assert!(!limiter.start_connection());

        // 结束一个连接
        limiter.end_connection();

        // 现在应该可以
        assert!(limiter.start_connection());
    }
}
