//! Load balancing strategies for service routing

mod health_aware;
mod least_connection;
mod round_robin;
mod weighted;

use async_trait::async_trait;
use std::sync::Arc;
use uhorse_discovery::ServiceInstance;

pub use health_aware::HealthAwareLoadBalancer;
pub use least_connection::LeastConnectionLoadBalancer;
pub use round_robin::RoundRobinLoadBalancer;
pub use weighted::WeightedLoadBalancer;

/// Load balancer trait
#[async_trait]
pub trait LoadBalancer: Send + Sync {
    /// Select a service instance
    async fn select(&self, instances: &[ServiceInstance]) -> Option<ServiceInstance>;

    /// Update instance statistics (for adaptive load balancers)
    async fn update_stats(&self, instance_id: &str, stats: InstanceStats);

    /// Get the name of this load balancer
    fn name(&self) -> &str;
}

/// Instance statistics for load balancing decisions
#[derive(Debug, Clone)]
pub struct InstanceStats {
    /// Active connections count
    pub active_connections: u32,
    /// Total requests served
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Last update timestamp
    pub last_updated: std::time::Instant,
}

impl Default for InstanceStats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: 0.0,
            last_updated: std::time::Instant::now(),
        }
    }
}

/// Load balancer factory
pub struct LoadBalancerFactory;

impl LoadBalancerFactory {
    /// Create a load balancer based on the strategy
    pub fn create(strategy: LoadBalanceStrategy) -> Arc<dyn LoadBalancer> {
        match strategy {
            LoadBalanceStrategy::RoundRobin => Arc::new(RoundRobinLoadBalancer::new()),
            LoadBalanceStrategy::Weighted => Arc::new(WeightedLoadBalancer::new()),
            LoadBalanceStrategy::HealthAware => Arc::new(HealthAwareLoadBalancer::new()),
            LoadBalanceStrategy::LeastConnection => Arc::new(LeastConnectionLoadBalancer::new()),
        }
    }
}

/// Load balance strategy enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadBalanceStrategy {
    /// Round-robin load balancing
    #[default]
    RoundRobin,
    /// Weighted round-robin based on instance weight
    Weighted,
    /// Health-aware load balancing (skip unhealthy instances)
    HealthAware,
    /// Least connection load balancing
    LeastConnection,
}

impl std::fmt::Display for LoadBalanceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadBalanceStrategy::RoundRobin => write!(f, "round-robin"),
            LoadBalanceStrategy::Weighted => write!(f, "weighted"),
            LoadBalanceStrategy::HealthAware => write!(f, "health-aware"),
            LoadBalanceStrategy::LeastConnection => write!(f, "least-connection"),
        }
    }
}

impl std::str::FromStr for LoadBalanceStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "round-robin" | "roundrobin" => Ok(LoadBalanceStrategy::RoundRobin),
            "weighted" => Ok(LoadBalanceStrategy::Weighted),
            "health-aware" | "healthaware" => Ok(LoadBalanceStrategy::HealthAware),
            "least-connection" | "leastconnection" => Ok(LoadBalanceStrategy::LeastConnection),
            _ => Err(format!("Unknown load balance strategy: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_load_balance_strategy_parse() {
        assert_eq!(
            LoadBalanceStrategy::from_str("round-robin").unwrap(),
            LoadBalanceStrategy::RoundRobin
        );
        assert_eq!(
            LoadBalanceStrategy::from_str("weighted").unwrap(),
            LoadBalanceStrategy::Weighted
        );
        assert!(LoadBalanceStrategy::from_str("invalid").is_err());
    }

    #[test]
    fn test_load_balance_strategy_display() {
        assert_eq!(LoadBalanceStrategy::RoundRobin.to_string(), "round-robin");
        assert_eq!(LoadBalanceStrategy::Weighted.to_string(), "weighted");
    }
}
