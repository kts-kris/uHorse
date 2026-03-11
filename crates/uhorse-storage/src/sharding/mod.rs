//! Database sharding module
//!
//! This module provides database sharding capabilities for horizontal scaling.

mod migration;
mod replica;
mod router;
mod strategy;

pub use migration::{MigrationManager, MigrationStatus};
pub use replica::{ReadWriteSplitter, ReplicaManager};
pub use router::{RouteResult, ShardingRouter};
pub use strategy::{ShardConfig, ShardKey, ShardingConfig, ShardingStrategy};
