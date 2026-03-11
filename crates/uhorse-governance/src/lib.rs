//! # uHorse Data Governance
//!
//! 数据治理模块，提供：
//! - 数据分类框架
//! - 保留策略
//! - 归档机制

pub mod archive;
pub mod classification;
pub mod retention;

pub use archive::{ArchiveManager, ArchiveRecord, ArchiveStats, ArchiveStatus};
pub use classification::{
    ClassificationResult, ClassificationRule, DataClassifier, DataType, SensitivityLevel,
};
pub use retention::{
    RetentionAction, RetentionCheckResult, RetentionPolicy, RetentionPolicyManager,
};

use thiserror::Error;

/// 数据治理错误
#[derive(Debug, Error)]
pub enum GovernanceError {
    /// 归档错误
    #[error("Archive error: {0}")]
    ArchiveError(String),

    /// 存储错误
    #[error("Storage error: {0}")]
    StorageError(#[from] anyhow::Error),

    /// 未找到
    #[error("Not found: {0}")]
    NotFound(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// 数据治理结果类型
pub type Result<T> = std::result::Result<T, GovernanceError>;
