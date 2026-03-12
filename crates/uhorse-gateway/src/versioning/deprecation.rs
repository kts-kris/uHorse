//! API Deprecation Management
//!
//! API 端点废弃管理和通知

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 废弃信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    /// 端点路径
    pub path: String,
    /// 废弃日期
    pub deprecated_at: DateTime<Utc>,
    /// 移除日期
    pub removal_date: DateTime<Utc>,
    /// 替代端点
    pub replacement: Option<String>,
    /// 迁移指南
    pub migration_guide: Option<String>,
    /// 是否已移除
    pub removed: bool,
}

impl DeprecationInfo {
    /// 创建新的废弃信息
    pub fn new(
        path: impl Into<String>,
        deprecated_at: DateTime<Utc>,
        removal_date: DateTime<Utc>,
    ) -> Self {
        Self {
            path: path.into(),
            deprecated_at,
            removal_date,
            replacement: None,
            migration_guide: None,
            removed: false,
        }
    }

    /// 设置替代端点
    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }

    /// 设置迁移指南
    pub fn with_migration_guide(mut self, guide: impl Into<String>) -> Self {
        self.migration_guide = Some(guide.into());
        self
    }

    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.removal_date
    }

    /// 获取剩余天数
    pub fn days_until_removal(&self) -> i64 {
        (self.removal_date - Utc::now()).num_days()
    }

    /// 获取废弃头信息
    pub fn to_header(&self) -> DeprecationHeader {
        let mut header = DeprecationHeader::new(
            self.deprecated_at,
            self.removal_date,
        );

        if let Some(ref replacement) = self.replacement {
            header = header.with_replacement(replacement.clone());
        }

        if let Some(ref guide) = self.migration_guide {
            header = header.with_link(guide.clone());
        }

        header
    }
}

/// 废弃响应头
#[derive(Debug, Clone)]
pub struct DeprecationHeader {
    /// 废弃日期
    pub deprecated_at: DateTime<Utc>,
    /// 移除日期
    pub removal_date: DateTime<Utc>,
    /// 替代端点
    pub replacement: Option<String>,
    /// 迁移指南链接
    pub link: Option<String>,
    /// 额外信息
    pub extra: HashMap<String, String>,
}

impl DeprecationHeader {
    /// 创建新的废弃头
    pub fn new(deprecated_at: DateTime<Utc>, removal_date: DateTime<Utc>) -> Self {
        Self {
            deprecated_at,
            removal_date,
            replacement: None,
            link: None,
            extra: HashMap::new(),
        }
    }

    /// 设置替代端点
    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }

    /// 设置迁移指南链接
    pub fn with_link(mut self, link: impl Into<String>) -> Self {
        self.link = Some(link.into());
        self
    }

    /// 添加额外信息
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// 转换为 HTTP 响应头
    pub fn to_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        // Deprecation 头 (RFC 8594)
        headers.push(("Deprecation".to_string(), "true".to_string()));

        // Sunset 头 (RFC 8594)
        let sunset = self.removal_date.to_rfc3339();
        headers.push(("Sunset".to_string(), sunset));

        // Link 头 (替代端点和迁移指南)
        if let Some(ref replacement) = self.replacement {
            let link = format!("<{}>; rel=\"successor-version\"", replacement);
            headers.push(("Link".to_string(), link));
        }

        if let Some(ref link_url) = self.link {
            let link = format!("<{}>; rel=\"deprecation\"", link_url);
            if let Some((_, existing)) = headers.iter().find(|(k, _)| k == "Link") {
                let combined = format!("{}, {}", existing, link);
                headers.retain(|(k, _)| k != "Link");
                headers.push(("Link".to_string(), combined));
            } else {
                headers.push(("Link".to_string(), link));
            }
        }

        headers
    }
}

/// 废弃管理器
pub struct DeprecationManager {
    /// 废弃端点列表
    deprecations: HashMap<String, DeprecationInfo>,
}

impl DeprecationManager {
    /// 创建新的废弃管理器
    pub fn new() -> Self {
        Self {
            deprecations: HashMap::new(),
        }
    }

    /// 添加废弃端点
    pub fn add(&mut self, info: DeprecationInfo) -> &mut Self {
        self.deprecations.insert(info.path.clone(), info);
        self
    }

    /// 获取废弃信息
    pub fn get(&self, path: &str) -> Option<&DeprecationInfo> {
        self.deprecations.get(path)
    }

    /// 检查端点是否已废弃
    pub fn is_deprecated(&self, path: &str) -> bool {
        self.deprecations.contains_key(path)
    }

    /// 检查端点是否已移除
    pub fn is_removed(&self, path: &str) -> bool {
        self.deprecations.get(path).map(|d| d.is_expired()).unwrap_or(false)
    }

    /// 获取废弃头
    pub fn get_headers(&self, path: &str) -> Option<Vec<(String, String)>> {
        self.deprecations.get(path).map(|d| d.to_header().to_headers())
    }

    /// 列出所有废弃端点
    pub fn list_all(&self) -> Vec<&DeprecationInfo> {
        self.deprecations.values().collect()
    }

    /// 列出已过期端点
    pub fn list_expired(&self) -> Vec<&DeprecationInfo> {
        self.deprecations.values().filter(|d| d.is_expired()).collect()
    }

    /// 列出即将移除的端点 (30 天内)
    pub fn list_upcoming(&self) -> Vec<&DeprecationInfo> {
        self.deprecations
            .values()
            .filter(|d| !d.is_expired() && d.days_until_removal() <= 30)
            .collect()
    }

    /// 标记端点为已移除
    pub fn mark_removed(&mut self, path: &str) -> bool {
        if let Some(info) = self.deprecations.get_mut(path) {
            info.removed = true;
            true
        } else {
            false
        }
    }
}

impl Default for DeprecationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deprecation_info() {
        let now = Utc::now();
        let removal = now + chrono::Duration::days(90);

        let info = DeprecationInfo::new("/api/v1/old", now, removal)
            .with_replacement("/api/v2/new")
            .with_migration_guide("https://docs.uhorse.ai/migration");

        assert!(!info.is_expired());
        assert!(info.days_until_removal() > 0);
        assert!(info.replacement.is_some());
    }

    #[test]
    fn test_deprecation_header() {
        let now = Utc::now();
        let removal = now + chrono::Duration::days(90);

        let header = DeprecationHeader::new(now, removal)
            .with_replacement("/api/v2/endpoint");

        let headers = header.to_headers();
        assert!(headers.iter().any(|(k, _)| k == "Deprecation"));
        assert!(headers.iter().any(|(k, _)| k == "Sunset"));
    }

    #[test]
    fn test_deprecation_manager() {
        let mut manager = DeprecationManager::new();

        let now = Utc::now();
        let removal = now + chrono::Duration::days(90);

        manager.add(
            DeprecationInfo::new("/api/v1/old", now, removal)
        );

        assert!(manager.is_deprecated("/api/v1/old"));
        assert!(!manager.is_removed("/api/v1/old"));
    }
}
