//! Request routing for sharded database

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::strategy::{ShardConfig, ShardKey, ShardingConfig, ShardingStrategy};

/// Route result containing target shard information
#[derive(Debug, Clone)]
pub struct RouteResult {
    /// Target shard ID
    pub shard_id: u32,
    /// Shard configuration
    pub shard: ShardConfig,
    /// Whether to use replica for read operations
    pub use_replica: bool,
}

/// Sharding router for database requests
pub struct ShardingRouter {
    /// Sharding configuration
    config: ShardingConfig,
    /// Shard lookup cache (for tenant-based sharding)
    tenant_shard_map: Arc<RwLock<HashMap<String, u32>>>,
    /// Consistent hash ring (for hash-based sharding)
    hash_ring: Arc<RwLock<Vec<u32>>>,
}

impl ShardingRouter {
    /// Create a new sharding router
    pub fn new(config: ShardingConfig) -> Self {
        let hash_ring = Self::build_hash_ring(&config);
        Self {
            config,
            tenant_shard_map: Arc::new(RwLock::new(HashMap::new())),
            hash_ring: Arc::new(RwLock::new(hash_ring)),
        }
    }

    /// Route a request to the appropriate shard
    pub async fn route(&self, key: &ShardKey, is_read: bool) -> Result<RouteResult> {
        let shard_id = match self.config.strategy {
            ShardingStrategy::TenantBased => self.route_by_tenant(key).await?,
            ShardingStrategy::HashBased => self.route_by_hash(key).await?,
            ShardingStrategy::RangeBased => self.route_by_range(key).await?,
        };

        let shard = self
            .config
            .get_shard(shard_id)
            .ok_or_else(|| anyhow!("Shard {} not found", shard_id))?
            .clone();

        if !shard.active {
            return Err(anyhow!("Shard {} is not active", shard_id));
        }

        debug!("Routed to shard {} for key {:?}", shard_id, key);

        Ok(RouteResult {
            shard_id,
            shard,
            use_replica: is_read,
        })
    }

    /// Route by tenant ID
    async fn route_by_tenant(&self, key: &ShardKey) -> Result<u32> {
        let tenant_id = key
            .tenant_id
            .as_ref()
            .ok_or_else(|| anyhow!("Tenant ID required for tenant-based sharding"))?;

        // Check cache first
        {
            let map = self.tenant_shard_map.read().await;
            if let Some(&shard_id) = map.get(tenant_id) {
                return Ok(shard_id);
            }
        }

        // Calculate shard assignment
        let shard_id = self.assign_shard_for_tenant(tenant_id).await?;

        // Cache the assignment
        {
            let mut map = self.tenant_shard_map.write().await;
            map.insert(tenant_id.clone(), shard_id);
        }

        Ok(shard_id)
    }

    /// Assign a shard for a new tenant
    async fn assign_shard_for_tenant(&self, tenant_id: &str) -> Result<u32> {
        let active_shards = self.config.active_shards();
        if active_shards.is_empty() {
            return Err(anyhow!("No active shards available"));
        }

        // Use consistent hash for assignment
        let hash = Self::hash_key(tenant_id);
        let ring = self.hash_ring.read().await;

        // Find the first shard in the ring with hash >= key hash
        let shard_id = ring
            .iter()
            .find(|&&id| {
                let shard_hash = Self::hash_key(&id.to_string());
                shard_hash >= hash
            })
            .copied()
            .unwrap_or_else(|| ring[0]);

        info!("Assigned shard {} for tenant {}", shard_id, tenant_id);
        Ok(shard_id)
    }

    /// Route by hash of user ID
    async fn route_by_hash(&self, key: &ShardKey) -> Result<u32> {
        let user_id = key
            .user_id
            .as_ref()
            .ok_or_else(|| anyhow!("User ID required for hash-based sharding"))?;

        let hash = Self::hash_key(user_id);
        let ring = self.hash_ring.read().await;

        // Binary search for the appropriate shard
        let shard_id = ring
            .iter()
            .find(|&&id| {
                let shard_hash = Self::hash_key(&id.to_string());
                shard_hash >= hash
            })
            .copied()
            .unwrap_or_else(|| ring[0]);

        Ok(shard_id)
    }

    /// Route by time range
    async fn route_by_range(&self, key: &ShardKey) -> Result<u32> {
        let timestamp = key
            .timestamp
            .ok_or_else(|| anyhow!("Timestamp required for range-based sharding"))?;

        // Use time-based partitioning (e.g., monthly shards)
        // For now, use modulo on month
        let month = (timestamp / (30 * 24 * 60 * 60)) as u32;
        let active_shards = self.config.active_shards();
        let shard_count = active_shards.len() as u32;

        let shard_id = month % shard_count;
        Ok(active_shards[shard_id as usize].id)
    }

    /// Build consistent hash ring
    fn build_hash_ring(config: &ShardingConfig) -> Vec<u32> {
        let mut ring: Vec<u32> = config
            .shards
            .iter()
            .filter(|s| s.active)
            .map(|s| s.id)
            .collect();
        ring.sort();
        ring
    }

    /// Hash a key using FNV-1a
    fn hash_key(key: &str) -> u64 {
        let mut hash: u64 = 0x811c9dc5;
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }

    /// Add a new shard assignment for a tenant
    pub async fn assign_tenant_shard(&self, tenant_id: &str, shard_id: u32) -> Result<()> {
        if self.config.get_shard(shard_id).is_none() {
            return Err(anyhow!("Shard {} does not exist", shard_id));
        }

        let mut map = self.tenant_shard_map.write().await;
        map.insert(tenant_id.to_string(), shard_id);

        info!(
            "Manually assigned shard {} for tenant {}",
            shard_id, tenant_id
        );
        Ok(())
    }

    /// Remove a tenant's shard assignment
    pub async fn remove_tenant_assignment(&self, tenant_id: &str) -> Result<()> {
        let mut map = self.tenant_shard_map.write().await;
        map.remove(tenant_id);

        info!("Removed shard assignment for tenant {}", tenant_id);
        Ok(())
    }

    /// Get all tenant assignments
    pub async fn get_tenant_assignments(&self) -> HashMap<String, u32> {
        self.tenant_shard_map.read().await.clone()
    }

    /// Rebuild the hash ring (after adding/removing shards)
    pub async fn rebuild_hash_ring(&mut self) {
        let ring = Self::build_hash_ring(&self.config);
        let mut current_ring = self.hash_ring.write().await;
        *current_ring = ring;

        info!("Rebuilt hash ring with {} shards", current_ring.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ShardingConfig {
        ShardingConfig {
            strategy: ShardingStrategy::TenantBased,
            shard_count: 3,
            shards: vec![
                ShardConfig {
                    id: 1,
                    dsn: "sqlite://shard1.db".to_string(),
                    weight: 1,
                    active: true,
                },
                ShardConfig {
                    id: 2,
                    dsn: "sqlite://shard2.db".to_string(),
                    weight: 1,
                    active: true,
                },
                ShardConfig {
                    id: 3,
                    dsn: "sqlite://shard3.db".to_string(),
                    weight: 1,
                    active: true,
                },
            ],
            replicas: vec![],
        }
    }

    #[tokio::test]
    async fn test_route_by_tenant() {
        let config = create_test_config();
        let router = ShardingRouter::new(config);

        let key = ShardKey::from_tenant("tenant-001");
        let result = router.route(&key, false).await.unwrap();
        assert!(result.shard_id >= 1 && result.shard_id <= 3);
    }

    #[tokio::test]
    async fn test_route_by_hash() {
        let mut config = create_test_config();
        config.strategy = ShardingStrategy::HashBased;
        let router = ShardingRouter::new(config);

        let key = ShardKey::from_user("user-123");
        let result = router.route(&key, false).await.unwrap();
        assert!(result.shard_id >= 1 && result.shard_id <= 3);
    }

    #[tokio::test]
    async fn test_tenant_assignment_caching() {
        let config = create_test_config();
        let router = ShardingRouter::new(config);

        // First routing
        let key = ShardKey::from_tenant("tenant-001");
        let result1 = router.route(&key, false).await.unwrap();

        // Second routing should return the same shard
        let result2 = router.route(&key, false).await.unwrap();
        assert_eq!(result1.shard_id, result2.shard_id);
    }

    #[tokio::test]
    async fn test_manual_tenant_assignment() {
        let config = create_test_config();
        let router = ShardingRouter::new(config);

        router
            .assign_tenant_shard("tenant-custom", 2)
            .await
            .unwrap();

        let key = ShardKey::from_tenant("tenant-custom");
        let result = router.route(&key, false).await.unwrap();
        assert_eq!(result.shard_id, 2);
    }

    #[tokio::test]
    async fn test_hash_key_consistency() {
        let hash1 = ShardingRouter::hash_key("test-key");
        let hash2 = ShardingRouter::hash_key("test-key");
        assert_eq!(hash1, hash2);
    }
}
