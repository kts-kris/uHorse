//! Least connection load balancing strategy

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{InstanceStats, LoadBalancer};

/// Least connection load balancer
pub struct LeastConnectionLoadBalancer {
    connections: Arc<RwLock<HashMap<String, u32>>>,
}

impl LeastConnectionLoadBalancer {
    /// Create a new least connection load balancer
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the current connection count for an instance
    async fn get_connections(&self, instance_id: &str) -> u32 {
        let connections = self.connections.read().await;
        connections.get(instance_id).copied().unwrap_or(0)
    }
}

impl Default for LeastConnectionLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoadBalancer for LeastConnectionLoadBalancer {
    async fn select(
        &self,
        instances: &[uhorse_discovery::ServiceInstance],
    ) -> Option<uhorse_discovery::ServiceInstance> {
        if instances.is_empty() {
            return None;
        }

        // Find instance with least connections
        let mut min_connections = u32::MAX;
        let mut selected_instance = None;

        for instance in instances {
            let connections = self.get_connections(&instance.id).await;
            if connections < min_connections {
                min_connections = connections;
                selected_instance = Some(instance.clone());
            }
        }

        // Increment connection count for selected instance
        if let Some(ref instance) = &selected_instance {
            let mut connections = self.connections.write().await;
            let count = connections.entry(instance.id.clone()).or_insert(0);
            *count += 1;
        }

        selected_instance
    }

    async fn update_stats(&self, instance_id: &str, stats: InstanceStats) {
        let mut connections = self.connections.write().await;
        connections.insert(instance_id.to_string(), stats.active_connections);
    }

    fn name(&self) -> &str {
        "least-connection"
    }
}

impl Drop for LeastConnectionLoadBalancer {
    fn drop(&mut self) {
        // Cleanup is handled by Arc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_least_connection_selection() {
        let lb = LeastConnectionLoadBalancer::new();

        let instances = vec![
            uhorse_discovery::ServiceInstance::new("1", "test", "10.0.0.1", 8080),
            uhorse_discovery::ServiceInstance::new("2", "test", "10.0.0.2", 8080),
            uhorse_discovery::ServiceInstance::new("3", "test", "10.0.0.3", 8080),
        ];

        // Set different connection counts
        lb.update_stats(
            "1",
            InstanceStats {
                active_connections: 10,
                ..Default::default()
            },
        )
        .await;

        lb.update_stats(
            "2",
            InstanceStats {
                active_connections: 5,
                ..Default::default()
            },
        )
        .await;

        lb.update_stats(
            "3",
            InstanceStats {
                active_connections: 20,
                ..Default::default()
            },
        )
        .await;

        // Should select instance 2 (least connections)
        let selected = lb.select(&instances).await.unwrap();
        assert_eq!(selected.id, "2");
    }

    #[tokio::test]
    async fn test_empty_instances() {
        let lb = LeastConnectionLoadBalancer::new();
        let result = lb.select(&[]).await;
        assert!(result.is_none());
    }
}
