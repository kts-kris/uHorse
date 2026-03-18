//! API Versioning Module
//!
//! API 版本管理，支持 URL 版本、版本废弃和兼容性检查

mod compat;
mod deprecation;
mod url;

pub use compat::{BreakingChange, CompatibilityChecker, CompatibilityLevel};
pub use deprecation::{DeprecationHeader, DeprecationInfo, DeprecationManager};
pub use url::{ApiVersion, VersionParser, VersionedPath};
