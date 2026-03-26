//! Sharding strategies for database partitioning

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Sharding strategy types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShardingStrategy {
    /// Shard by tenant ID
    /// Each tenant's data is stored in a specific shard
    #[default]
    TenantBased,
    /// Shard by hash of user ID
    /// Distributes data evenly across shards using consistent hashing
    HashBased,
    /// Shard by time range
    /// Useful for time-series data or log-based storage
    RangeBased,
}

impl std::str::FromStr for ShardingStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "tenant_based" | "tenant" => Ok(Self::TenantBased),
            "hash_based" | "hash" => Ok(Self::HashBased),
            "range_based" | "range" => Ok(Self::RangeBased),
            _ => Err(anyhow!("Unknown sharding strategy: {}", s)),
        }
    }
}

/// Shard key for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardKey {
    /// Tenant ID (for tenant-based sharding)
    pub tenant_id: Option<String>,
    /// User ID (for hash-based sharding)
    pub user_id: Option<String>,
    /// Timestamp (for range-based sharding)
    pub timestamp: Option<i64>,
}

impl ShardKey {
    /// Create a tenant-based shard key
    pub fn from_tenant(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: Some(tenant_id.into()),
            user_id: None,
            timestamp: None,
        }
    }

    /// Create a user-based shard key
    pub fn from_user(user_id: impl Into<String>) -> Self {
        Self {
            tenant_id: None,
            user_id: Some(user_id.into()),
            timestamp: None,
        }
    }

    /// Create a time-based shard key
    pub fn from_timestamp(timestamp: i64) -> Self {
        Self {
            tenant_id: None,
            user_id: None,
            timestamp: Some(timestamp),
        }
    }
}

/// Shard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    /// Unique shard identifier
    pub id: u32,
    /// Database connection string
    pub dsn: String,
    /// Optional weight for weighted distribution
    #[serde(default)]
    pub weight: u32,
    /// Whether this shard is currently active
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}

/// Replica configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaConfig {
    /// Shard ID this replica belongs to
    pub shard_id: u32,
    /// Replica identifier
    pub replica_id: u32,
    /// Database connection string
    pub dsn: String,
    /// Whether this replica is read-only
    #[serde(default = "default_true")]
    pub read_only: bool,
}

/// Sharding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardingConfig {
    /// Sharding strategy
    pub strategy: ShardingStrategy,
    /// Total number of shards
    pub shard_count: u32,
    /// Shard configurations
    pub shards: Vec<ShardConfig>,
    /// Replica configurations
    #[serde(default)]
    pub replicas: Vec<ReplicaConfig>,
}

impl Default for ShardingConfig {
    fn default() -> Self {
        Self {
            strategy: ShardingStrategy::default(),
            shard_count: 1,
            shards: vec![ShardConfig {
                id: 0,
                dsn: "sqlite://data/uhorse.db".to_string(),
                weight: 1,
                active: true,
            }],
            replicas: vec![],
        }
    }
}

impl ShardingConfig {
    /// Load configuration from TOML string
    pub fn from_toml(toml: &str) -> Result<Self> {
        let config: Self = toml::from_str(toml)?;
        Ok(config)
    }

    /// Get shard by ID
    pub fn get_shard(&self, id: u32) -> Option<&ShardConfig> {
        self.shards.iter().find(|s| s.id == id)
    }

    /// Get replicas for a shard
    pub fn get_replicas(&self, shard_id: u32) -> Vec<&ReplicaConfig> {
        self.replicas
            .iter()
            .filter(|r| r.shard_id == shard_id)
            .collect()
    }

    /// Get active shards
    pub fn active_shards(&self) -> Vec<&ShardConfig> {
        self.shards.iter().filter(|s| s.active).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_sharding_strategy_from_str() {
        assert_eq!(
            ShardingStrategy::from_str("tenant_based").unwrap(),
            ShardingStrategy::TenantBased
        );
        assert_eq!(
            ShardingStrategy::from_str("hash").unwrap(),
            ShardingStrategy::HashBased
        );
        assert_eq!(
            ShardingStrategy::from_str("range_based").unwrap(),
            ShardingStrategy::RangeBased
        );
    }

    #[test]
    fn test_shard_key_creation() {
        let key = ShardKey::from_tenant("tenant-001");
        assert_eq!(key.tenant_id, Some("tenant-001".to_string()));
        assert_eq!(key.user_id, None);

        let key = ShardKey::from_user("user-123");
        assert_eq!(key.user_id, Some("user-123".to_string()));

        let key = ShardKey::from_timestamp(1234567890);
        assert_eq!(key.timestamp, Some(1234567890));
    }

    #[test]
    fn test_sharding_config_default() {
        let config = ShardingConfig::default();
        assert_eq!(config.strategy, ShardingStrategy::TenantBased);
        assert_eq!(config.shard_count, 1);
        assert_eq!(config.shards.len(), 1);
    }

    #[test]
    fn test_sharding_config_from_toml() {
        let toml = r#"
            strategy = "hash_based"
            shard_count = 2

            [[shards]]
            id = 1
            dsn = "sqlite://data/shard1.db"
            weight = 1

            [[shards]]
            id = 2
            dsn = "sqlite://data/shard2.db"
            weight = 2
        "#;

        let config = ShardingConfig::from_toml(toml).unwrap();
        assert_eq!(config.strategy, ShardingStrategy::HashBased);
        assert_eq!(config.shards.len(), 2);
    }
}
