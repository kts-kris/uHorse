//! Retry strategies for failed tasks

use std::time::Duration;

/// Retry strategy configuration
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// No retries
    None,
    /// Fixed interval between retries
    Fixed {
        /// Interval between retries
        interval: Duration,
        /// Maximum number of retries
        max_retries: u32,
    },
    /// Exponential backoff
    ExponentialBackoff {
        /// Initial interval
        initial_interval: Duration,
        /// Multiplier for each retry
        multiplier: f64,
        /// Maximum interval cap
        max_interval: Duration,
        /// Maximum number of retries
        max_retries: u32,
    },
    /// Custom intervals
    Custom {
        /// List of intervals to use
        intervals: Vec<Duration>,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::exponential_backoff(3)
    }
}

impl RetryStrategy {
    /// Create a no-retry strategy
    pub fn none() -> Self {
        Self::None
    }

    /// Create a fixed interval retry strategy
    pub fn fixed(max_retries: u32, interval: Duration) -> Self {
        Self::Fixed { interval, max_retries }
    }

    /// Create an exponential backoff strategy
    pub fn exponential_backoff(max_retries: u32) -> Self {
        Self::ExponentialBackoff {
            initial_interval: Duration::from_millis(100),
            multiplier: 2.0,
            max_interval: Duration::from_secs(60),
            max_retries,
        }
    }

    /// Create a custom intervals strategy
    pub fn custom(intervals: Vec<Duration>) -> Self {
        Self::Custom { intervals }
    }

    /// Get the delay for a given attempt
    pub fn get_delay(&self, attempt: u32) -> Option<Duration> {
        match self {
            Self::None => None,
            Self::Fixed { interval, max_retries } => {
                if attempt < *max_retries {
                    Some(*interval)
                } else {
                    None
                }
            }
            Self::ExponentialBackoff {
                initial_interval,
                multiplier,
                max_interval,
                max_retries,
            } => {
                if attempt < *max_retries {
                    let delay = initial_interval.as_secs_f64()
                        * multiplier.powi(attempt as i32);
                    let delay = delay.min(max_interval.as_secs_f64());
                    Some(Duration::from_secs_f64(delay))
                } else {
                    None
                }
            }
            Self::Custom { intervals } => {
                intervals.get(attempt as usize).copied()
            }
        }
    }

    /// Check if more retries are available
    pub fn has_retry(&self, attempt: u32) -> bool {
        self.get_delay(attempt).is_some()
    }

    /// Get maximum retries
    pub fn max_retries(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Fixed { max_retries, .. } => *max_retries,
            Self::ExponentialBackoff { max_retries, .. } => *max_retries,
            Self::Custom { intervals } => intervals.len() as u32,
        }
    }
}

/// Retry policy with state tracking
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Retry strategy
    pub strategy: RetryStrategy,
    /// Jitter to add randomness (0.0 - 1.0)
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryPolicy {
    /// Create a new retry policy with default settings
    pub fn new() -> Self {
        Self {
            strategy: RetryStrategy::default(),
            jitter: 0.1,
        }
    }

    /// Set retry strategy
    pub fn with_strategy(mut self, strategy: RetryStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set jitter factor
    pub fn with_jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Calculate the next delay with jitter
    pub fn calculate_delay(&self, attempt: u32) -> Option<Duration> {
        let base_delay = self.strategy.get_delay(attempt)?;

        // Add jitter: delay * (1 - jitter/2 + random * jitter)
        use rand::Rng;
        let random_value: f64 = rand::thread_rng().gen();
        let jitter_factor = 1.0 - self.jitter / 2.0 + (random_value * self.jitter);

        Some(Duration::from_secs_f64(base_delay.as_secs_f64() * jitter_factor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_retry() {
        let strategy = RetryStrategy::none();
        assert!(!strategy.has_retry(0));
        assert!(!strategy.has_retry(1));
    }

    #[test]
    fn test_fixed_retry() {
        let strategy = RetryStrategy::fixed(3, Duration::from_secs(1));

        assert!(strategy.has_retry(0));
        assert!(strategy.has_retry(2));
        assert!(!strategy.has_retry(3));

        assert_eq!(strategy.get_delay(0), Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::exponential_backoff(3);

        let d0 = strategy.get_delay(0).unwrap();
        let d1 = strategy.get_delay(1).unwrap();
        let d2 = strategy.get_delay(2).unwrap();

        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn test_custom_intervals() {
        let strategy = RetryStrategy::custom(vec![
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(500),
        ]);

        assert_eq!(strategy.get_delay(0), Some(Duration::from_millis(100)));
        assert_eq!(strategy.get_delay(1), Some(Duration::from_millis(200)));
        assert_eq!(strategy.get_delay(2), Some(Duration::from_millis(500)));
        assert_eq!(strategy.get_delay(3), None);
    }
}
