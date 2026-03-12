//! API Versioning Module
//!
//! API 版本管理，支持 URL 版本、版本废弃和兼容性检查

mod url;
mod deprecation;
mod compat;

pub use url::{ApiVersion, VersionedPath, VersionParser};
pub use deprecation::{DeprecationInfo, DeprecationManager, DeprecationHeader};
pub use compat::{CompatibilityLevel, CompatibilityChecker, BreakingChange};
