//! Weighted round-robin load balancing strategy

use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{LoadBalancer, InstanceStats};

/// Weighted round-robin load balancer
pub struct WeightedLoadBalancer {
    counter: AtomicU64,
}

impl WeightedLoadBalancer {
    /// Create a new weighted load balancer
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Get the weight for an instance (default to 50 if not specified)
    fn get_weight(instance: &uhorse_discovery::ServiceInstance) -> u32 {
        instance.metadata.weight.unwrap_or(50)
    }
}

impl Default for WeightedLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoadBalancer for WeightedLoadBalancer {
    async fn select(&self, instances: &[uhorse_discovery::ServiceInstance]) -> Option<uhorse_discovery::ServiceInstance> {
        if instances.is_empty() {
            return None;
        }

        // Calculate total weight
        let total_weight: u64 = instances
            .iter()
            .map(|i| Self::get_weight(i) as u64)
            .sum();

        if total_weight == 0 {
            // Fall back to round-robin if all weights are 0
            let index = self.counter.fetch_add(1, Ordering::Relaxed);
            return Some(instances[index as usize % instances.len()].clone());
        }

        // Get a random position within total weight
        let position = self.counter.fetch_add(1, Ordering::Relaxed) % total_weight;

        // Find the instance at this position
        let mut current_weight = 0u64;
        for instance in instances {
            current_weight += Self::get_weight(instance) as u64;
            if position < current_weight {
                return Some(instance.clone());
            }
        }

        // Should not reach here, but return last instance as fallback
        instances.last().cloned()
    }

    async fn update_stats(&self, _instance_id: &str, _stats: InstanceStats) {
        // Weighted load balancer doesn't use dynamic stats
    }

    fn name(&self) -> &str {
        "weighted"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_weighted_selection() {
        let lb = WeightedLoadBalancer::new();

        let instances = vec![
            uhorse_discovery::ServiceInstance::new("1", "test", "10.0.0.1", 8080)
                .with_weight(10), // 10% traffic
            uhorse_discovery::ServiceInstance::new("2", "test", "10.0.0.2", 8080)
                .with_weight(30), // 30% traffic
            uhorse_discovery::ServiceInstance::new("3", "test", "10.0.0.3", 8080)
                .with_weight(60), // 60% traffic
        ];

        // Run multiple selections and count distribution
        let mut counts = std::collections::HashMap::new();
        for _ in 0..1000 {
            let selected = lb.select(&instances).await.unwrap();
            *counts.entry(selected.id).or_insert(0) += 1;
        }

        // Instance 3 with weight 60 should get approximately 60% of traffic
        let count3 = counts.get("3").unwrap_or(&0);
        assert!(*count3 > 500 && *count3 < 700, "Expected ~600 for instance 3, got {}", count3);
    }

    #[tokio::test]
    async fn test_empty_instances() {
        let lb = WeightedLoadBalancer::new();
        let result = lb.select(&[]).await;
        assert!(result.is_none());
    }
}
