//! Data migration utilities for database sharding

use anyhow::Result;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Migration status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationStatus {
    /// Migration is pending
    Pending,
    /// Migration is in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed with an error message
    Failed(String),
    /// Migration was rolled back
    RolledBack,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(e) => write!(f, "Failed: {}", e),
            Self::RolledBack => write!(f, "Rolled Back"),
        }
    }
}

/// Migration step
#[derive(Debug, Clone)]
pub struct MigrationStep {
    /// Unique identifier for this migration step
    pub id: String,
    /// Source shard ID
    pub source_shard: u32,
    /// Target shard ID
    pub target_shard: u32,
    /// Data range start (inclusive)
    pub range_start: i64,
    /// Data range end (exclusive)
    pub range_end: i64,
    /// Current status
    pub status: MigrationStatus,
    /// Number of records processed
    pub processed: u64,
    /// Total number of records to process
    pub total: u64,
    /// When the migration started (timestamp in seconds)
    pub started_at: Option<i64>,
    /// When the migration completed (timestamp in seconds)
    pub completed_at: Option<i64>,
}

impl MigrationStep {
    /// Create a new migration step
    pub fn new(
        id: impl Into<String>,
        source_shard: u32,
        target_shard: u32,
        range_start: i64,
        range_end: i64,
        total: u64,
    ) -> Self {
        Self {
            id: id.into(),
            source_shard,
            target_shard,
            range_start,
            range_end,
            status: MigrationStatus::Pending,
            processed: 0,
            total,
            started_at: None,
            completed_at: None,
        }
    }

    /// Get the progress percentage (0-100)
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.processed as f64 / self.total as f64) * 100.0
    }

    /// Check if the migration is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, MigrationStatus::Completed)
    }

    /// Check if the migration is still running
    pub fn is_running(&self) -> bool {
        matches!(self.status, MigrationStatus::InProgress)
    }
}

/// Migration manager for data rebalancing across shards
pub struct MigrationManager {
    /// Active migrations
    migrations: Arc<RwLock<Vec<MigrationStep>>>,
}

impl Default for MigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationManager {
    /// Create a new migration manager
    pub fn new() -> Self {
        Self {
            migrations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new migration
    pub async fn create_migration(&self, step: MigrationStep) -> Result<()> {
        let mut migrations = self.migrations.write().await;
        if migrations.iter().any(|m| m.id == step.id) {
            return Err(anyhow::anyhow!("Migration {} already exists", step.id));
        }
        migrations.push(step);
        Ok(())
    }

    /// Get a migration by ID
    pub async fn get_migration(&self, id: &str) -> Option<MigrationStep> {
        let migrations = self.migrations.read().await;
        migrations.iter().find(|m| m.id == id).cloned()
    }

    /// Update a migration status
    pub async fn update_status(
        &self,
        id: &str,
        status: MigrationStatus,
        processed: Option<u64>,
    ) -> Result<()> {
        let mut migrations = self.migrations.write().await;
        if let Some(migration) = migrations.iter_mut().find(|m| m.id == id) {
            migration.status = status.clone();
            if let Some(p) = processed {
                migration.processed = p;
            }
            if matches!(status, MigrationStatus::InProgress) && migration.started_at.is_none() {
                migration.started_at = Some(chrono::Utc::now().timestamp());
            }
            if matches!(status, MigrationStatus::Completed) {
                migration.completed_at = Some(chrono::Utc::now().timestamp());
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Migration {} not found", id))
        }
    }

    /// Get all migrations for a specific shard
    pub async fn get_migrations_for_shard(&self, shard_id: u32) -> Vec<MigrationStep> {
        let migrations = self.migrations.read().await;
        migrations
            .iter()
            .filter(|m| m.source_shard == shard_id || m.target_shard == shard_id)
            .cloned()
            .collect()
    }

    /// Cancel a pending or failed migration
    pub async fn cancel_migration(&self, id: &str) -> Result<()> {
        let mut migrations = self.migrations.write().await;
        if let Some(pos) = migrations.iter().position(|m| m.id == id) {
            let migration = migrations.remove(pos);
            if matches!(migration.status, MigrationStatus::InProgress) {
                return Err(anyhow::anyhow!("Cannot cancel in-progress migration"));
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Migration {} not found", id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sharding::strategy::ShardingConfig;

    #[tokio::test]
    async fn test_migration_step_creation() {
        let step = MigrationStep::new("test-migration", 1, 2, 0, 1000, 1000);
        assert_eq!(step.id, "test-migration");
        assert_eq!(step.source_shard, 1);
        assert_eq!(step.target_shard, 2);
        assert_eq!(step.status, MigrationStatus::Pending);
        assert_eq!(step.progress(), 0.0);
    }

    #[tokio::test]
    async fn test_migration_progress() {
        let mut step = MigrationStep::new("test", 1, 2, 0, 1000, 1000);
        assert_eq!(step.progress(), 0.0);

        step.processed = 500;
        assert_eq!(step.progress(), 50.0);

        step.processed = 1000;
        assert_eq!(step.progress(), 100.0);
    }

    #[tokio::test]
    async fn test_migration_manager() {
        let manager = MigrationManager::new();
        let step = MigrationStep::new("test", 1, 2, 0, 1000, 1000);

        manager.create_migration(step).await.unwrap();
        let result = manager.get_migration("test").await.unwrap();
        assert!(result.is_some());

        let migrations = manager.get_migrations_for_shard(1).await;
        assert_eq!(migrations.len(), 1);
    }
}
