//! Read-write splitting and replica management

use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::strategy::ReplicaConfig;

/// Statistics for a replica
#[derive(Debug, Default)]
pub struct ReplicaStats {
    /// Total queries served
    pub queries_served: AtomicU64,
    /// Failed queries
    pub queries_failed: AtomicU64,
    /// Average latency in microseconds
    pub avg_latency_us: AtomicU64,
    /// Last health check timestamp
    pub last_health_check: AtomicU64,
}

/// Replica state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplicaState {
    /// Replica is healthy and accepting reads
    Healthy,
    /// Replica is lagging behind
    Lagging,
    /// Replica is unhealthy
    Unhealthy,
    /// Replica is disconnected
    Disconnected,
}

/// Replica information
#[derive(Debug)]
pub struct ReplicaInfo {
    /// Replica configuration
    pub config: ReplicaConfig,
    /// Current state
    pub state: ReplicaState,
    /// Statistics
    pub stats: ReplicaStats,
    /// Replication lag in seconds
    pub lag_seconds: f64,
}

/// Replica manager for read-write splitting
pub struct ReplicaManager {
    /// Replica information by shard
    replicas: Arc<RwLock<Vec<ReplicaInfo>>>,
    /// Maximum allowed lag for reads (seconds)
    max_lag_seconds: f64,
}

impl ReplicaManager {
    /// Create a new replica manager
    pub fn new(replica_configs: Vec<ReplicaConfig>, max_lag_seconds: f64) -> Self {
        let replicas = replica_configs
            .into_iter()
            .map(|config| ReplicaInfo {
                config,
                state: ReplicaState::Healthy,
                stats: ReplicaStats::default(),
                lag_seconds: 0.0,
            })
            .collect();

        Self {
            replicas: Arc::new(RwLock::new(replicas)),
            max_lag_seconds,
        }
    }

    /// Select a replica for read operations
    pub async fn select_replica(&self, shard_id: u32) -> Result<Option<ReplicaConfig>> {
        let replicas = self.replicas.read().await;

        // Find healthy replicas for this shard
        let healthy_replicas: Vec<_> = replicas
            .iter()
            .filter(|r| {
                r.config.shard_id == shard_id
                    && r.config.read_only
                    && r.state == ReplicaState::Healthy
                    && r.lag_seconds <= self.max_lag_seconds
            })
            .collect();

        if healthy_replicas.is_empty() {
            debug!("No healthy replicas available for shard {}", shard_id);
            return Ok(None);
        }

        // Round-robin selection with weighted random
        let selected = self.weighted_select(&healthy_replicas);
        let config = selected.config.clone();

        // Update stats
        selected
            .stats
            .queries_served
            .fetch_add(1, Ordering::Relaxed);

        debug!(
            "Selected replica {} for shard {}",
            config.replica_id, shard_id
        );
        Ok(Some(config))
    }

    /// Weighted selection based on latency and health
    fn weighted_select<'a>(&self, replicas: &'a [&ReplicaInfo]) -> &'a ReplicaInfo {
        if replicas.len() == 1 {
            return replicas[0];
        }

        // Calculate weights based on inverse latency
        let weights: Vec<u64> = replicas
            .iter()
            .map(|r| {
                let latency = r.stats.avg_latency_us.load(Ordering::Relaxed).max(1);
                // Higher weight for lower latency
                1_000_000 / latency
            })
            .collect();

        let total_weight: u64 = weights.iter().sum();
        let mut random_value = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64)
            % total_weight;

        for (i, weight) in weights.iter().enumerate() {
            if random_value < *weight {
                return replicas[i];
            }
            random_value -= weight;
        }

        replicas[0]
    }

    /// Update replica health status
    pub async fn update_health(&self, replica_id: u32, lag_seconds: f64, is_healthy: bool) {
        let mut replicas = self.replicas.write().await;

        if let Some(replica) = replicas
            .iter_mut()
            .find(|r| r.config.replica_id == replica_id)
        {
            replica.lag_seconds = lag_seconds;

            let new_state = if !is_healthy {
                ReplicaState::Unhealthy
            } else if lag_seconds > self.max_lag_seconds {
                ReplicaState::Lagging
            } else {
                ReplicaState::Healthy
            };

            if replica.state != new_state {
                info!(
                    "Replica {} state changed: {:?} -> {:?}",
                    replica_id, replica.state, new_state
                );
                replica.state = new_state;
            }

            replica.stats.last_health_check.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                Ordering::Relaxed,
            );
        }
    }

    /// Record a query result
    pub async fn record_query(&self, replica_id: u32, success: bool, latency_us: u64) {
        let replicas = self.replicas.read().await;

        if let Some(replica) = replicas.iter().find(|r| r.config.replica_id == replica_id) {
            if success {
                replica.stats.queries_served.fetch_add(1, Ordering::Relaxed);
            } else {
                replica.stats.queries_failed.fetch_add(1, Ordering::Relaxed);
            }

            // Update average latency (simple moving average)
            let current = replica.stats.avg_latency_us.load(Ordering::Relaxed);
            let new_avg = if current == 0 {
                latency_us
            } else {
                (current * 9 + latency_us) / 10
            };
            replica
                .stats
                .avg_latency_us
                .store(new_avg, Ordering::Relaxed);
        }
    }

    /// Get all replicas for a shard
    pub async fn get_replicas(&self, shard_id: u32) -> Vec<ReplicaInfo> {
        let replicas = self.replicas.read().await;
        replicas
            .iter()
            .filter(|r| r.config.shard_id == shard_id)
            .map(|r| ReplicaInfo {
                config: r.config.clone(),
                state: r.state,
                stats: ReplicaStats {
                    queries_served: AtomicU64::new(r.stats.queries_served.load(Ordering::Relaxed)),
                    queries_failed: AtomicU64::new(r.stats.queries_failed.load(Ordering::Relaxed)),
                    avg_latency_us: AtomicU64::new(r.stats.avg_latency_us.load(Ordering::Relaxed)),
                    last_health_check: AtomicU64::new(
                        r.stats.last_health_check.load(Ordering::Relaxed),
                    ),
                },
                lag_seconds: r.lag_seconds,
            })
            .collect()
    }

    /// Get healthy replica count
    pub async fn healthy_count(&self, shard_id: u32) -> usize {
        let replicas = self.replicas.read().await;
        replicas
            .iter()
            .filter(|r| r.config.shard_id == shard_id && r.state == ReplicaState::Healthy)
            .count()
    }
}

/// Read-write splitter for database operations
pub struct ReadWriteSplitter {
    /// Replica manager
    replica_manager: Arc<ReplicaManager>,
    /// Whether to use replicas for reads
    use_replicas: bool,
}

impl ReadWriteSplitter {
    /// Create a new read-write splitter
    pub fn new(replica_manager: Arc<ReplicaManager>) -> Self {
        Self {
            replica_manager,
            use_replicas: true,
        }
    }

    /// Disable replica usage
    pub fn disable_replicas(&mut self) {
        self.use_replicas = false;
        warn!("Replica usage disabled - all reads will go to primary");
    }

    /// Enable replica usage
    pub fn enable_replicas(&mut self) {
        self.use_replicas = true;
        info!("Replica usage enabled");
    }

    /// Determine if a replica should be used for this operation
    pub async fn should_use_replica(&self, shard_id: u32, is_read: bool) -> bool {
        if !is_read || !self.use_replicas {
            return false;
        }

        // Check if healthy replicas are available
        self.replica_manager.healthy_count(shard_id).await > 0
    }

    /// Get the data source for an operation
    pub async fn get_datasource(
        &self,
        shard_id: u32,
        primary_dsn: &str,
        is_read: bool,
    ) -> Result<String> {
        if self.should_use_replica(shard_id, is_read).await {
            if let Some(replica) = self.replica_manager.select_replica(shard_id).await? {
                debug!(
                    "Using replica {} for shard {}",
                    replica.replica_id, shard_id
                );
                return Ok(replica.dsn);
            }
        }

        // Fall back to primary
        debug!("Using primary for shard {}", shard_id);
        Ok(primary_dsn.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_replicas() -> Vec<ReplicaConfig> {
        vec![
            ReplicaConfig {
                shard_id: 1,
                replica_id: 1,
                dsn: "sqlite://replica1.db".to_string(),
                read_only: true,
            },
            ReplicaConfig {
                shard_id: 1,
                replica_id: 2,
                dsn: "sqlite://replica2.db".to_string(),
                read_only: true,
            },
        ]
    }

    #[tokio::test]
    async fn test_replica_manager_creation() {
        let configs = create_test_replicas();
        let manager = ReplicaManager::new(configs, 10.0);

        let replicas = manager.get_replicas(1).await;
        assert_eq!(replicas.len(), 2);
    }

    #[tokio::test]
    async fn test_select_replica() {
        let configs = create_test_replicas();
        let manager = ReplicaManager::new(configs, 10.0);

        let replica = manager.select_replica(1).await.unwrap();
        assert!(replica.is_some());
        assert!(replica.unwrap().read_only);
    }

    #[tokio::test]
    async fn test_replica_health_update() {
        let configs = create_test_replicas();
        let manager = ReplicaManager::new(configs, 10.0);

        manager.update_health(1, 5.0, true).await;

        let replicas = manager.get_replicas(1).await;
        assert_eq!(replicas[0].state, ReplicaState::Healthy);
    }

    #[tokio::test]
    async fn test_replica_lagging_state() {
        let configs = create_test_replicas();
        let manager = ReplicaManager::new(configs, 5.0);

        manager.update_health(1, 10.0, true).await;

        let replicas = manager.get_replicas(1).await;
        assert_eq!(replicas[0].state, ReplicaState::Lagging);
    }

    #[tokio::test]
    async fn test_read_write_splitter() {
        let configs = create_test_replicas();
        let manager = Arc::new(ReplicaManager::new(configs, 10.0));
        let splitter = ReadWriteSplitter::new(manager);

        // Write should use primary
        let dsn = splitter
            .get_datasource(1, "primary.db", false)
            .await
            .unwrap();
        assert_eq!(dsn, "primary.db");

        // Read should use replica
        let dsn = splitter
            .get_datasource(1, "primary.db", true)
            .await
            .unwrap();
        assert!(dsn.contains("replica"));
    }

    #[tokio::test]
    async fn test_record_query_stats() {
        let configs = create_test_replicas();
        let manager = ReplicaManager::new(configs, 10.0);

        manager.record_query(1, true, 100).await;

        let replicas = manager.get_replicas(1).await;
        assert_eq!(replicas[0].stats.queries_served.load(Ordering::Relaxed), 1);
    }
}
