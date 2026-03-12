//! Endpoint Rate Limiter
//!
//! 端点级别的细粒度限流

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::{RateLimitAlgorithm, RateLimitConfig, RateLimitResult};

/// 端点限流配置
#[derive(Debug, Clone)]
pub struct EndpointRateLimitConfig {
    /// 基础配置
    pub base: RateLimitConfig,
    /// 默认限制
    pub default_limit: EndpointLimit,
}

impl Default for EndpointRateLimitConfig {
    fn default() -> Self {
        Self {
            base: RateLimitConfig::default(),
            default_limit: EndpointLimit::default(),
        }
    }
}

/// 端点限制
#[derive(Debug, Clone)]
pub struct EndpointLimit {
    /// 端点模式 (支持通配符)
    pub pattern: String,
    /// HTTP 方法 (为空表示所有方法)
    pub method: Option<String>,
    /// 时间窗口 (秒)
    pub window_secs: u64,
    /// 最大请求数
    pub max_requests: u64,
    /// 描述
    pub description: Option<String>,
}

impl EndpointLimit {
    /// 创建新的端点限制
    pub fn new(pattern: impl Into<String>, window_secs: u64, max_requests: u64) -> Self {
        Self {
            pattern: pattern.into(),
            method: None,
            window_secs,
            max_requests,
            description: None,
        }
    }

    /// 设置 HTTP 方法
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 匹配端点
    pub fn matches(&self, method: &str, path: &str) -> bool {
        // 检查方法
        if let Some(ref m) = self.method {
            if m != "*" && m.to_lowercase() != method.to_lowercase() {
                return false;
            }
        }

        // 检查路径模式
        self.match_pattern(&self.pattern, path)
    }

    /// 简单的模式匹配
    fn match_pattern(&self, pattern: &str, path: &str) -> bool {
        if pattern == "*" || pattern == path {
            return true;
        }

        // 支持通配符前缀
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return path.starts_with(prefix);
        }

        // 支持路径参数
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        for (p, actual) in pattern_parts.iter().zip(path_parts.iter()) {
            if *p == ":*" || p.starts_with(':') {
                continue;
            }
            if p != actual {
                return false;
            }
        }

        true
    }
}

impl Default for EndpointLimit {
    fn default() -> Self {
        Self::new("/*", 60, 100)
    }
}

/// 端点状态
#[derive(Debug, Clone)]
struct EndpointState {
    /// 窗口开始时间
    window_start: Instant,
    /// 当前请求数
    current_requests: u64,
    /// 最大请求数
    max_requests: u64,
    /// 窗口大小 (秒)
    window_secs: u64,
}

/// 端点限流器
pub struct EndpointRateLimiter {
    /// 配置
    config: EndpointRateLimitConfig,
    /// 端点限制列表
    limits: Vec<EndpointLimit>,
    /// 端点状态
    states: Arc<RwLock<HashMap<String, EndpointState>>>,
}

impl EndpointRateLimiter {
    /// 创建新的端点限流器
    pub fn new(config: EndpointRateLimitConfig) -> Self {
        Self {
            config,
            limits: Vec::new(),
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加端点限制
    pub fn add_limit(&mut self, limit: EndpointLimit) -> &mut Self {
        self.limits.push(limit);
        self
    }

    /// 检查是否允许请求
    pub async fn check(&self, method: &str, path: &str, client_id: &str) -> RateLimitResult {
        if !self.config.base.enabled {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
                reset_after: 0,
            };
        }

        // 查找匹配的限制
        let limit = self.limits.iter()
            .find(|l| l.matches(method, path))
            .unwrap_or(&self.config.default_limit);

        let key = format!("{}:{}:{}", client_id, method, path);
        let now = Instant::now();
        let window_duration = Duration::from_secs(limit.window_secs);

        let mut states = self.states.write().await;

        // 获取或创建状态
        let state = states.entry(key.clone()).or_insert_with(|| EndpointState {
            window_start: now,
            current_requests: 0,
            max_requests: limit.max_requests,
            window_secs: limit.window_secs,
        });

        // 检查是否需要重置窗口
        if now.duration_since(state.window_start) >= window_duration {
            state.window_start = now;
            state.current_requests = 0;
            state.max_requests = limit.max_requests;
            state.window_secs = limit.window_secs;
        }

        // 检查限制
        if state.current_requests >= state.max_requests {
            let elapsed = now.duration_since(state.window_start);
            let reset_after = window_duration.saturating_sub(elapsed).as_secs();

            return RateLimitResult::Denied {
                retry_after: reset_after.max(1),
                limit: state.max_requests,
            };
        }

        // 增加计数
        state.current_requests += 1;
        let remaining = state.max_requests - state.current_requests;

        let elapsed = now.duration_since(state.window_start);
        let reset_after = window_duration.saturating_sub(elapsed).as_secs();

        RateLimitResult::Allowed {
            remaining,
            reset_after,
        }
    }

    /// 获取端点状态
    pub async fn get_status(&self, method: &str, path: &str, client_id: &str) -> Option<EndpointStatus> {
        let key = format!("{}:{}:{}", client_id, method, path);
        let states = self.states.read().await;

        states.get(&key).map(|s| EndpointStatus {
            current_requests: s.current_requests,
            max_requests: s.max_requests,
            remaining: s.max_requests.saturating_sub(s.current_requests),
            window_secs: s.window_secs,
        })
    }

    /// 重置端点状态
    pub async fn reset(&self, method: &str, path: &str, client_id: &str) -> bool {
        let key = format!("{}:{}:{}", client_id, method, path);
        let mut states = self.states.write().await;

        if let Some(state) = states.get_mut(&key) {
            state.current_requests = 0;
            state.window_start = Instant::now();
            true
        } else {
            false
        }
    }

    /// 列出所有限制
    pub fn list_limits(&self) -> &[EndpointLimit] {
        &self.limits
    }
}

impl Default for EndpointRateLimiter {
    fn default() -> Self {
        Self::new(EndpointRateLimitConfig::default())
    }
}

/// 端点状态
#[derive(Debug, Clone)]
pub struct EndpointStatus {
    /// 当前请求数
    pub current_requests: u64,
    /// 最大请求数
    pub max_requests: u64,
    /// 剩余配额
    pub remaining: u64,
    /// 窗口大小 (秒)
    pub window_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_limit_matching() {
        let limit = EndpointLimit::new("/api/v1/agents", 60, 100);

        assert!(limit.matches("GET", "/api/v1/agents"));
        assert!(limit.matches("POST", "/api/v1/agents"));
        assert!(!limit.matches("GET", "/api/v1/agents/123"));
    }

    #[test]
    fn test_endpoint_limit_with_method() {
        let limit = EndpointLimit::new("/api/v1/agents", 60, 100)
            .with_method("GET");

        assert!(limit.matches("GET", "/api/v1/agents"));
        assert!(!limit.matches("POST", "/api/v1/agents"));
    }

    #[test]
    fn test_endpoint_limit_wildcard() {
        let limit = EndpointLimit::new("/api/v1/*", 60, 100);

        assert!(limit.matches("GET", "/api/v1/agents"));
        assert!(limit.matches("GET", "/api/v1/sessions"));
        assert!(!limit.matches("GET", "/api/v2/agents"));
    }

    #[tokio::test]
    async fn test_endpoint_rate_limiter() {
        let mut limiter = EndpointRateLimiter::default();
        limiter.add_limit(EndpointLimit::new("/api/v1/test", 60, 5));

        // 前 5 个请求应该成功
        for _ in 0..5 {
            let result = limiter.check("GET", "/api/v1/test", "client-1").await;
            assert!(result.is_allowed());
        }

        // 第 6 个请求应该被拒绝
        let result = limiter.check("GET", "/api/v1/test", "client-1").await;
        assert!(!result.is_allowed());
    }
}
