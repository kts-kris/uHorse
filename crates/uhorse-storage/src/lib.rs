//! # uHorse Storage
//!
//! 存储层，提供 SQLite 和 JSONL 持久化，以及数据库分片支持

pub mod jsonl;
pub mod migration;
pub mod secret;
pub mod sharding;
pub mod sqlite;

pub use jsonl::JsonlLogger;
pub use sharding::{
    MigrationManager, MigrationStatus, ReadWriteSplitter, ReplicaManager, RouteResult,
    ShardingRouter, ShardConfig, ShardKey, ShardingConfig, ShardingStrategy,
};
pub use sqlite::SqliteStore;

/// Storage result type
pub type Result<T> = anyhow::Result<T>;
