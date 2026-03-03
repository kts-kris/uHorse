//! # uHorse Security
//!
//! 安全层，提供认证、设备配对、审批流程和幂等性保证。

pub mod approval;
pub mod auth;
pub mod idempotency;
pub mod pairing;

pub use approval::{
    ApprovalLevel, ApprovalManager, ApprovalRequest, ApprovalRuleEngine, ApprovalStatus,
};
pub use auth::{JwtAuthService, TokenPair};
pub use idempotency::IdempotencyCache;
pub use pairing::{DevicePairingManager, PairingRequest, PairingStatus};
