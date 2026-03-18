//! Round-robin load balancing strategy

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{InstanceStats, LoadBalancer};

/// Round-robin load balancer
pub struct RoundRobinLoadBalancer {
    counter: AtomicUsize,
}

impl RoundRobinLoadBalancer {
    /// Create a new round-robin load balancer
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoadBalancer for RoundRobinLoadBalancer {
    async fn select(
        &self,
        instances: &[uhorse_discovery::ServiceInstance],
    ) -> Option<uhorse_discovery::ServiceInstance> {
        if instances.is_empty() {
            return None;
        }

        let index = self.counter.fetch_add(1, Ordering::Relaxed);
        Some(instances[index % instances.len()].clone())
    }

    async fn update_stats(&self, _instance_id: &str, _stats: InstanceStats) {
        // Round-robin doesn't use stats
    }

    fn name(&self) -> &str {
        "round-robin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_round_robin_selection() {
        let lb = RoundRobinLoadBalancer::new();

        let instances = vec![
            uhorse_discovery::ServiceInstance::new("1", "test", "10.0.0.1", 8080),
            uhorse_discovery::ServiceInstance::new("2", "test", "10.0.0.2", 8080),
            uhorse_discovery::ServiceInstance::new("3", "test", "10.0.0.3", 8080),
        ];

        // Test round-robin selection
        let first = lb.select(&instances).await.unwrap();
        let second = lb.select(&instances).await.unwrap();
        let third = lb.select(&instances).await.unwrap();
        let fourth = lb.select(&instances).await.unwrap();

        // Should cycle through instances
        assert_eq!(first.id, "1");
        assert_eq!(second.id, "2");
        assert_eq!(third.id, "3");
        assert_eq!(fourth.id, "1"); // Wraps around
    }

    #[tokio::test]
    async fn test_empty_instances() {
        let lb = RoundRobinLoadBalancer::new();
        let result = lb.select(&[]).await;
        assert!(result.is_none());
    }
}
