//! # uHorse Security
//!
//! 安全层，提供认证、设备配对、审批流程和幂等性保证。

pub mod auth;
pub mod pairing;
pub mod approval;
pub mod idempotency;

pub use auth::{JwtAuthService, TokenPair};
pub use pairing::{DevicePairingManager, PairingRequest, PairingStatus};
pub use approval::{ApprovalManager, ApprovalRequest, ApprovalStatus, ApprovalLevel, ApprovalRuleEngine};
pub use idempotency::IdempotencyCache;
