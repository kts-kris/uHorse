//! Health-aware load balancing strategy

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{InstanceStats, LoadBalancer};
use crate::lb::RoundRobinLoadBalancer;

/// Health-aware load balancer that filters out unhealthy instances
pub struct HealthAwareLoadBalancer {
    inner: RoundRobinLoadBalancer,
    health_status: Arc<RwLock<HashMap<String, bool>>>,
}

impl HealthAwareLoadBalancer {
    /// Create a new health-aware load balancer
    pub fn new() -> Self {
        Self {
            inner: RoundRobinLoadBalancer::new(),
            health_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if an instance is healthy
    async fn is_healthy(&self, instance_id: &str) -> bool {
        let status = self.health_status.read().await;
        status.get(instance_id).copied().unwrap_or(true) // Assume healthy if no status
    }
}

impl Default for HealthAwareLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoadBalancer for HealthAwareLoadBalancer {
    async fn select(
        &self,
        instances: &[uhorse_discovery::ServiceInstance],
    ) -> Option<uhorse_discovery::ServiceInstance> {
        if instances.is_empty() {
            return None;
        }

        // Filter healthy instances
        let mut healthy_instances = Vec::new();
        for instance in instances {
            if self.is_healthy(&instance.id).await {
                healthy_instances.push(instance.clone());
            }
        }

        if healthy_instances.is_empty() {
            tracing::warn!("No healthy instances available");
            return None;
        }

        // Use round-robin on healthy instances
        self.inner.select(&healthy_instances).await
    }

    async fn update_stats(&self, instance_id: &str, stats: InstanceStats) {
        // Update health status based on failure rate
        let is_healthy = if stats.total_requests > 0 {
            let failure_rate = stats.failed_requests as f64 / stats.total_requests as f64;
            failure_rate < 0.5 // Consider unhealthy if >50% failures
        } else {
            true // No requests yet, assume healthy
        };

        let mut status = self.health_status.write().await;
        status.insert(instance_id.to_string(), is_healthy);
    }

    fn name(&self) -> &str {
        "health-aware"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_aware_selection() {
        let lb = HealthAwareLoadBalancer::new();

        let instances = vec![
            uhorse_discovery::ServiceInstance::new("1", "test", "10.0.0.1", 8080),
            uhorse_discovery::ServiceInstance::new("2", "test", "10.0.0.2", 8080),
            uhorse_discovery::ServiceInstance::new("3", "test", "10.0.0.3", 8080),
        ];

        // All should be healthy initially
        let selected = lb.select(&instances).await;
        assert!(selected.is_some());

        // Mark instance 2 as unhealthy
        lb.update_stats(
            "2",
            InstanceStats {
                total_requests: 100,
                failed_requests: 60,
                ..Default::default()
            },
        )
        .await;

        // Should not select instance 2
        for _ in 0..100 {
            let selected = lb.select(&instances).await.unwrap();
            assert_ne!(selected.id, "2");
        }
    }

    #[tokio::test]
    async fn test_all_unhealthy() {
        let lb = HealthAwareLoadBalancer::new();

        let instances = vec![uhorse_discovery::ServiceInstance::new(
            "1", "test", "10.0.0.1", 8080,
        )];

        // Mark as unhealthy
        lb.update_stats(
            "1",
            InstanceStats {
                total_requests: 100,
                failed_requests: 80,
                ..Default::default()
            },
        )
        .await;

        // Should return None when all are unhealthy
        let result = lb.select(&instances).await;
        assert!(result.is_none());
    }
}
