//! # uHorse GDPR/CCPA Compliance
//!
//! 数据隐私合规模块，提供：
//! - 用户数据导出 (Data Export)
//! - 数据删除/被遗忘权 (Right to be Forgotten)
//! - 同意管理 (Consent Management)
//! - 数据分类 (Data Classification)

pub mod classification;
pub mod consent;
pub mod erasure;
pub mod export;

pub use classification::{DataCategory, DataSensitivity, PersonalDataClassifier};
pub use consent::{ConsentManager, ConsentRecord, ConsentStatus, ConsentType, ProcessingPurpose};
pub use erasure::{DataErasureManager, ErasureRequest, ErasureStatus, ErasureVerification};
pub use export::{DataExportFormat, DataExportManager, DataExportRequest, ExportResult};

/// GDPR 合规错误类型
#[derive(Debug, thiserror::Error)]
pub enum GdprError {
    /// 用户未找到
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// 同意未授予
    #[error("Consent not granted for: {0}")]
    ConsentNotGranted(String),

    /// 数据导出失败
    #[error("Data export failed: {0}")]
    ExportFailed(String),

    /// 数据删除失败
    #[error("Data erasure failed: {0}")]
    ErasureFailed(String),

    /// 验证失败
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// 存储错误
    #[error("Storage error: {0}")]
    StorageError(#[from] anyhow::Error),
}

/// GDPR 合规结果类型
pub type Result<T> = std::result::Result<T, GdprError>;
