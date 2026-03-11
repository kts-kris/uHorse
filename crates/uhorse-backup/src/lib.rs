//! # uHorse Backup and Recovery
//!
//! 备份恢复模块，提供：
//! - 自动备份调度
//! - 备份加密
//! - 恢复工具
//! - 跨区域复制

pub mod encryption;
pub mod replication;
pub mod restore;
pub mod scheduler;

pub use encryption::{EncryptionConfig, EncryptionKey, EncryptionManager, EncryptedData};
pub use replication::{ReplicationConfig, ReplicationManager, ReplicationStatus, ReplicationStats, ReplicationTarget, ReplicationTask};
pub use restore::{RestoreConfig, RestoreManager, RestoreRecord, RestoreStats, RestoreStatus, RollbackInfo};
pub use scheduler::{BackupRecord, BackupScheduleConfig, BackupScheduler, BackupStats, BackupStatus, BackupType};

use thiserror::Error;

/// 备份错误
#[derive(Debug, Error)]
pub enum BackupError {
    /// 备份失败
    #[error("Backup failed: {0}")]
    BackupFailed(String),

    /// 恢复失败
    #[error("Restore failed: {0}")]
    RestoreFailed(String),

    /// 加密错误
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    /// 复制错误
    #[error("Replication error: {0}")]
    ReplicationError(String),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// 未找到
    #[error("Not found: {0}")]
    NotFound(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// 恢复错误 (已弃用)
    #[error("Restore error: {0}")]
    RestoreError(String),
}

/// 备份结果类型
pub type Result<T> = std::result::Result<T, BackupError>;
