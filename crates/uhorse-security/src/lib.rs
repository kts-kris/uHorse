//! # uHorse Security
//!
//! 安全层，提供认证、设备配对、审批流程、幂等性保证、TLS 加密和字段级加密。

pub mod approval;
pub mod auth;
pub mod field_crypto;
pub mod idempotency;
pub mod pairing;
pub mod tls;

pub use approval::{
    ApprovalLevel, ApprovalManager, ApprovalRequest, ApprovalRuleEngine, ApprovalStatus,
};
pub use auth::{JwtAuthService, TokenPair};
pub use field_crypto::{
    DataClassification, EncryptedField, EncryptionKey, FieldEncryptor, KeyId, KeyManager,
    KeyVersion, SensitiveField,
};
pub use idempotency::IdempotencyCache;
pub use pairing::{DevicePairingManager, PairingRequest, PairingStatus};
pub use tls::{
    CertificateManager, CipherSuite, HttpsRedirectConfig, TlsConfig, TlsServerBuilder, TlsVersion,
};
