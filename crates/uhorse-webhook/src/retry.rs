//! Retry Mechanism
//!
//! 实现指数退避重试机制

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// 重试策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始延迟 (毫秒)
    pub initial_delay_ms: u64,
    /// 最大延迟 (毫秒)
    pub max_delay_ms: u64,
    /// 退避倍数
    pub multiplier: f64,
    /// 抖动因子 (0.0 - 1.0)
    pub jitter: f64,
    /// 可重试的 HTTP 状态码
    pub retryable_status_codes: Vec<u16>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 60000,
            multiplier: 2.0,
            jitter: 0.1,
            retryable_status_codes: vec![408, 429, 500, 502, 503, 504],
        }
    }
}

impl RetryPolicy {
    /// 创建新的重试策略
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// 计算下一次重试的延迟时间
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        use rand::Rng;

        let base_delay = self.initial_delay_ms as f64
            * self.multiplier.powi(attempt.saturating_sub(1) as i32);

        // 应用最大延迟限制
        let delay = base_delay.min(self.max_delay_ms as f64);

        // 添加抖动
        let jitter_range = delay * self.jitter;
        let mut rng = rand::thread_rng();
        let jitter_value = rng.gen_range(-jitter_range..jitter_range);
        let final_delay = (delay + jitter_value).max(0.0) as u64;

        Duration::from_millis(final_delay)
    }

    /// 检查是否应该重试
    pub fn should_retry(&self, attempt: u32, error: &RetryableError) -> bool {
        if attempt >= self.max_retries {
            return false;
        }

        match error {
            RetryableError::Timeout => true,
            RetryableError::ConnectionError => true,
            RetryableError::HttpStatus(code) => self.retryable_status_codes.contains(code),
            RetryableError::Other(_) => false,
        }
    }

    /// 执行带重试的异步操作
    pub async fn execute<F, Fut, T, E>(
        &self,
        mut operation: F,
    ) -> Result<T, RetryState>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: Into<RetryableError>,
    {
        let mut attempt = 0;
        let mut delays_ms: Vec<u64> = Vec::new();

        loop {
            attempt += 1;

            match operation().await {
                Ok(result) => {
                    return Ok(result);
                }
                Err(error) => {
                    let error = error.into();

                    if !self.should_retry(attempt, &error) {
                        return Err(RetryState {
                            attempts: attempt,
                            last_error: Some(error.to_string()),
                            delays_ms,
                            exhausted: true,
                        });
                    }

                    warn!(
                        "Attempt {} failed: {:?}, retrying...",
                        attempt, error
                    );

                    let delay = self.calculate_delay(attempt);
                    delays_ms.push(delay.as_millis() as u64);

                    sleep(delay).await;
                }
            }
        }
    }
}

/// 可重试的错误类型
#[derive(Debug, Clone)]
pub enum RetryableError {
    /// 超时
    Timeout,
    /// 连接错误
    ConnectionError,
    /// HTTP 状态码
    HttpStatus(u16),
    /// 其他错误
    Other(String),
}

impl std::fmt::Display for RetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryableError::Timeout => write!(f, "Timeout"),
            RetryableError::ConnectionError => write!(f, "Connection error"),
            RetryableError::HttpStatus(code) => write!(f, "HTTP status: {}", code),
            RetryableError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl From<reqwest::Error> for RetryableError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            RetryableError::Timeout
        } else if error.is_connect() {
            RetryableError::ConnectionError
        } else if let Some(status) = error.status() {
            RetryableError::HttpStatus(status.as_u16())
        } else {
            RetryableError::Other(error.to_string())
        }
    }
}

/// 重试状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryState {
    /// 尝试次数
    pub attempts: u32,
    /// 最后的错误消息
    pub last_error: Option<String>,
    /// 延迟历史 (毫秒)
    pub delays_ms: Vec<u64>,
    /// 是否已耗尽重试次数
    pub exhausted: bool,
}

impl RetryState {
    /// 创建新的重试状态
    pub fn new() -> Self {
        Self {
            attempts: 0,
            last_error: None,
            delays_ms: Vec::new(),
            exhausted: false,
        }
    }

    /// 获取总延迟时间
    pub fn total_delay(&self) -> Duration {
        Duration::from_millis(self.delays_ms.iter().sum())
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay_ms, 1000);
    }

    #[test]
    fn test_calculate_delay() {
        let policy = RetryPolicy::default();

        let delay1 = policy.calculate_delay(1);
        let delay2 = policy.calculate_delay(2);
        let delay3 = policy.calculate_delay(3);

        // 延迟应该递增 (考虑抖动，不能严格比较)
        // 初始延迟约为 1000ms
        assert!(delay1 >= Duration::from_millis(500));
        assert!(delay1 <= Duration::from_millis(2000));

        // 延迟不应超过最大值 + 抖动
        let delay_max = policy.calculate_delay(100);
        let max_with_jitter = Duration::from_millis((policy.max_delay_ms as f64 * (1.0 + policy.jitter)) as u64);
        assert!(delay_max <= max_with_jitter);
    }

    #[test]
    fn test_should_retry() {
        let policy = RetryPolicy::default();

        // 可重试的错误
        assert!(policy.should_retry(1, &RetryableError::Timeout));
        assert!(policy.should_retry(1, &RetryableError::HttpStatus(503)));

        // 不可重试的错误
        assert!(!policy.should_retry(1, &RetryableError::HttpStatus(400)));
        assert!(!policy.should_retry(1, &RetryableError::Other("error".to_string())));

        // 超过最大次数
        assert!(!policy.should_retry(10, &RetryableError::Timeout));
    }

    #[tokio::test]
    async fn test_execute_with_retry() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let policy = RetryPolicy::new(3);
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = policy
            .execute(move || {
                let attempts = attempts_clone.clone();
                async move {
                    let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 2 {
                        Err(RetryableError::Timeout)
                    } else {
                        Ok("success")
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
