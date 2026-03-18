//! Health checking for service instances

use crate::types::{HealthCheckConfig, HealthStatus, ServiceInstance};

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Instance being checked
    pub instance_id: String,
    /// Health status
    pub status: HealthStatus,
    /// Timestamp of the check (Unix timestamp)
    pub timestamp: i64,
    /// Error message if unhealthy
    pub error: Option<String>,
    /// Response time in milliseconds
    pub response_time_ms: Option<u64>,
}

/// Health checker for service instances
pub struct HealthChecker {
    config: HealthCheckConfig,
    #[cfg(feature = "health-check")]
    client: reqwest::Client,
}

impl HealthChecker {
    /// Create a new health checker with the given configuration
    pub fn new(config: HealthCheckConfig) -> Self {
        #[cfg(feature = "health-check")]
        let timeout_secs = config.timeout_secs;
        Self {
            config,
            #[cfg(feature = "health-check")]
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Check the health of a service instance
    #[cfg(feature = "health-check")]
    pub async fn check(
        &self,
        instance: &ServiceInstance,
    ) -> crate::error::Result<HealthCheckResult> {
        let url = format!("{}{}", instance.http_url(), self.config.path);
        let start = std::time::Instant::now();

        let result = self.client.get(&url).send().await;
        let response_time_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                let status = if response.status().is_success() {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Unhealthy
                };

                Ok(HealthCheckResult {
                    instance_id: instance.id.clone(),
                    status,
                    timestamp: chrono::Utc::now().timestamp(),
                    error: if status == HealthStatus::Unhealthy {
                        Some(format!("HTTP {}", response.status()))
                    } else {
                        None
                    },
                    response_time_ms: Some(response_time_ms),
                })
            }
            Err(e) => Ok(HealthCheckResult {
                instance_id: instance.id.clone(),
                status: HealthStatus::Unhealthy,
                timestamp: chrono::Utc::now().timestamp(),
                error: Some(e.to_string()),
                response_time_ms: Some(response_time_ms),
            }),
        }
    }

    /// Check health without HTTP client (stub implementation)
    #[cfg(not(feature = "health-check"))]
    pub async fn check(
        &self,
        instance: &ServiceInstance,
    ) -> crate::error::Result<HealthCheckResult> {
        Ok(HealthCheckResult {
            instance_id: instance.id.clone(),
            status: HealthStatus::Unknown,
            timestamp: chrono::Utc::now().timestamp(),
            error: Some("health-check feature not enabled".to_string()),
            response_time_ms: None,
        })
    }

    /// Get the health check configuration
    pub fn config(&self) -> &HealthCheckConfig {
        &self.config
    }
}

/// Health status tracker for tracking consecutive failures/successes
#[derive(Debug, Default)]
pub struct HealthTracker {
    consecutive_failures: u32,
    consecutive_successes: u32,
    current_status: HealthStatus,
}

impl HealthTracker {
    /// Create a new health tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Update health status based on check result
    pub fn update(&mut self, is_healthy: bool, config: &HealthCheckConfig) -> HealthStatus {
        if is_healthy {
            self.consecutive_failures = 0;
            self.consecutive_successes += 1;

            if self.consecutive_successes >= config.success_threshold {
                self.current_status = HealthStatus::Healthy;
            }
        } else {
            self.consecutive_successes = 0;
            self.consecutive_failures += 1;

            if self.consecutive_failures >= config.failure_threshold {
                self.current_status = HealthStatus::Unhealthy;
            }
        }

        self.current_status
    }

    /// Get current health status
    pub fn status(&self) -> HealthStatus {
        self.current_status
    }

    /// Reset the tracker
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.consecutive_successes = 0;
        self.current_status = HealthStatus::Unknown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_tracker() {
        let config = HealthCheckConfig {
            failure_threshold: 3,
            success_threshold: 2,
            ..Default::default()
        };

        let mut tracker = HealthTracker::new();
        assert_eq!(tracker.status(), HealthStatus::Unknown);

        // Failures
        tracker.update(false, &config);
        tracker.update(false, &config);
        assert_eq!(tracker.status(), HealthStatus::Unknown);

        tracker.update(false, &config);
        assert_eq!(tracker.status(), HealthStatus::Unhealthy);

        // Successes
        tracker.update(true, &config);
        assert_eq!(tracker.status(), HealthStatus::Unhealthy);

        tracker.update(true, &config);
        assert_eq!(tracker.status(), HealthStatus::Healthy);
    }
}
