//! URL Version Management
//!
//! 基于 URL 的 API 版本管理 (/api/v1, /api/v2)

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// API 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    /// 主版本号
    pub major: u32,
    /// 次版本号
    pub minor: u32,
    /// 补丁版本号
    pub patch: u32,
}

impl ApiVersion {
    /// 创建新版本
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// v1 版本
    pub const fn v1() -> Self {
        Self::new(1, 0, 0)
    }

    /// v2 版本
    pub const fn v2() -> Self {
        Self::new(2, 0, 0)
    }

    /// 解析版本字符串
    pub fn parse(s: &str) -> Result<Self, VersionParseError> {
        let s = s.trim().trim_start_matches('v').trim_start_matches('V');

        let parts: Vec<&str> = s.split('.').collect();
        match parts.len() {
            1 => {
                let major: u32 = parts[0]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                Ok(Self::new(major, 0, 0))
            }
            2 => {
                let major: u32 = parts[0]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                let minor: u32 = parts[1]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                Ok(Self::new(major, minor, 0))
            }
            3 => {
                let major: u32 = parts[0]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                let minor: u32 = parts[1]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                let patch: u32 = parts[2]
                    .parse()
                    .map_err(|_| VersionParseError::InvalidFormat)?;
                Ok(Self::new(major, minor, patch))
            }
            _ => Err(VersionParseError::InvalidFormat),
        }
    }

    /// 转换为 URL 路径格式
    pub fn to_path(&self) -> String {
        format!("v{}", self.major)
    }

    /// 检查是否兼容指定版本
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        // 主版本相同则兼容
        self.major == other.major
    }

    /// 检查是否已废弃
    pub fn is_deprecated(&self, current: &Self) -> bool {
        self.major < current.major
    }
}

impl Default for ApiVersion {
    fn default() -> Self {
        Self::v1()
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for ApiVersion {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// 版本解析错误
#[derive(Debug, thiserror::Error)]
pub enum VersionParseError {
    #[error("Invalid version format")]
    InvalidFormat,

    #[error("Invalid version number")]
    InvalidNumber,
}

/// 版本化路径
#[derive(Debug, Clone)]
pub struct VersionedPath {
    /// API 版本
    pub version: ApiVersion,
    /// 原始路径
    pub path: String,
}

impl VersionedPath {
    /// 创建版本化路径
    pub fn new(version: ApiVersion, path: impl Into<String>) -> Self {
        Self {
            version,
            path: path.into(),
        }
    }

    /// 从 URL 路径解析
    pub fn from_url(url: &str) -> Option<Self> {
        // 匹配 /api/vX/... 格式
        let parts: Vec<&str> = url.trim_start_matches('/').split('/').collect();

        if parts.len() < 2 {
            return None;
        }

        // 检查是否是 /api/vX 格式
        if parts[0] == "api" && parts[1].starts_with('v') {
            let version = ApiVersion::parse(parts[1]).ok()?;
            let rest = parts[2..].join("/");
            let path = if rest.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", rest)
            };
            Some(Self::new(version, path))
        } else {
            None
        }
    }

    /// 转换为完整 URL 路径
    pub fn to_url(&self) -> String {
        format!(
            "/api/{}/{}",
            self.version.to_path(),
            self.path.trim_start_matches('/')
        )
    }
}

/// 版本解析器
pub struct VersionParser {
    /// 支持的版本列表
    supported_versions: Vec<ApiVersion>,
    /// 当前默认版本
    default_version: ApiVersion,
}

impl VersionParser {
    /// 创建版本解析器
    pub fn new(supported_versions: Vec<ApiVersion>, default_version: ApiVersion) -> Self {
        Self {
            supported_versions,
            default_version,
        }
    }

    /// 创建默认解析器 (支持 v1 和 v2)
    pub fn default_parser() -> Self {
        Self::new(vec![ApiVersion::v1(), ApiVersion::v2()], ApiVersion::v1())
    }

    /// 解析请求路径中的版本
    pub fn parse_version(&self, path: &str) -> Option<ApiVersion> {
        VersionedPath::from_url(path).map(|vp| vp.version)
    }

    /// 检查版本是否支持
    pub fn is_version_supported(&self, version: &ApiVersion) -> bool {
        self.supported_versions
            .iter()
            .any(|v| v.major == version.major)
    }

    /// 获取默认版本
    pub fn default_version(&self) -> &ApiVersion {
        &self.default_version
    }

    /// 获取所有支持的版本
    pub fn supported_versions(&self) -> &[ApiVersion] {
        &self.supported_versions
    }

    /// 重写路径到指定版本
    pub fn rewrite_path(&self, path: &str, target_version: &ApiVersion) -> Option<String> {
        let versioned = VersionedPath::from_url(path)?;
        Some(VersionedPath::new(target_version.clone(), versioned.path).to_url())
    }
}

impl Default for VersionParser {
    fn default() -> Self {
        Self::default_parser()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version_creation() {
        let v1 = ApiVersion::v1();
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 0);

        let v2 = ApiVersion::v2();
        assert_eq!(v2.major, 2);
    }

    #[test]
    fn test_api_version_parse() {
        let v = ApiVersion::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        let v = ApiVersion::parse("1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
    }

    #[test]
    fn test_api_version_compatible() {
        let v1_0 = ApiVersion::new(1, 0, 0);
        let v1_1 = ApiVersion::new(1, 1, 0);
        let v2_0 = ApiVersion::new(2, 0, 0);

        assert!(v1_0.is_compatible_with(&v1_1));
        assert!(!v1_0.is_compatible_with(&v2_0));
    }

    #[test]
    fn test_versioned_path() {
        let path = VersionedPath::from_url("/api/v1/agents").unwrap();
        assert_eq!(path.version, ApiVersion::v1());
        assert_eq!(path.path, "/agents");

        let url = path.to_url();
        assert_eq!(url, "/api/v1/agents");
    }

    #[test]
    fn test_version_parser() {
        let parser = VersionParser::default();

        assert!(parser.is_version_supported(&ApiVersion::v1()));
        assert!(parser.is_version_supported(&ApiVersion::v2()));
        assert_eq!(parser.default_version(), &ApiVersion::v1());
    }
}
