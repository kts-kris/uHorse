//! Consent management for GDPR compliance
//!
//! 用户同意管理，支持多种同意类型和撤销

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use super::Result;

/// 同意类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsentType {
    /// 数据处理同意
    DataProcessing,
    /// 营销通信同意
    Marketing,
    /// 分析使用同意
    Analytics,
    /// 第三方共享同意
    ThirdPartySharing,
    /// Cookie 使用同意
    CookieUsage,
    /// 位置数据同意
    LocationData,
    /// 个人资料收集
    Profiling,
}

impl std::fmt::Display for ConsentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataProcessing => write!(f, "data_processing"),
            Self::Marketing => write!(f, "marketing"),
            Self::Analytics => write!(f, "analytics"),
            Self::ThirdPartySharing => write!(f, "third_party_sharing"),
            Self::CookieUsage => write!(f, "cookie_usage"),
            Self::LocationData => write!(f, "location_data"),
            Self::Profiling => write!(f, "profiling"),
        }
    }
}

/// 处理目的
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessingPurpose {
    /// 目的 ID
    pub id: String,
    /// 目的名称
    pub name: String,
    /// 目的描述
    pub description: String,
    /// 法律依据
    pub legal_basis: LegalBasis,
    /// 是否必需
    pub is_required: bool,
    /// 保留期限 (天)
    pub retention_days: Option<u32>,
}

/// 法律依据 (GDPR Article 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegalBasis {
    /// 同意 (Article 6(1)(a))
    Consent,
    /// 合同履行 (Article 6(1)(b))
    ContractPerformance,
    /// 法律义务 (Article 6(1)(c))
    LegalObligation,
    /// 重大利益 (Article 6(1)(d))
    VitalInterests,
    /// 公共利益 (Article 6(1)(e))
    PublicInterest,
    /// 正当利益 (Article 6(1)(f))
    LegitimateInterest,
}

/// 同意状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsentStatus {
    /// 已授予
    Granted,
    /// 已拒绝
    Denied,
    /// 已撤销
    Withdrawn,
    /// 待确认
    Pending,
    /// 已过期
    Expired,
}

/// 同意记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// 记录 ID
    pub id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 同意类型
    pub consent_type: ConsentType,
    /// 状态
    pub status: ConsentStatus,
    /// 授予时间
    pub granted_at: Option<u64>,
    /// 撤销时间
    pub withdrawn_at: Option<u64>,
    /// 来源 (web/app/api)
    pub source: String,
    /// IP 地址
    pub ip_address: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 版本 (同意条款版本)
    pub version: String,
    /// 创建时间
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
}

impl ConsentRecord {
    /// 创建新的同意记录
    pub fn new(
        user_id: impl Into<String>,
        tenant_id: impl Into<String>,
        consent_type: ConsentType,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            tenant_id: tenant_id.into(),
            consent_type,
            status: ConsentStatus::Pending,
            granted_at: None,
            withdrawn_at: None,
            source: "unknown".to_string(),
            ip_address: None,
            user_agent: None,
            version: "1.0".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 授予同意
    pub fn grant(mut self, source: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        self.status = ConsentStatus::Granted;
        self.granted_at = Some(now);
        self.withdrawn_at = None;
        self.source = source.into();
        self.updated_at = now;
        self
    }

    /// 撤销同意
    pub fn withdraw(&mut self) {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        self.status = ConsentStatus::Withdrawn;
        self.withdrawn_at = Some(now);
        self.updated_at = now;
    }

    /// 检查是否有效
    pub fn is_valid(&self) -> bool {
        self.status == ConsentStatus::Granted
    }
}

/// 同意管理器
pub struct ConsentManager {
    /// 同意记录存储 (user_id -> consent_type -> record)
    records: Arc<RwLock<HashMap<String, HashMap<ConsentType, ConsentRecord>>>>,
    /// 处理目的
    purposes: HashMap<String, ProcessingPurpose>,
}

impl ConsentManager {
    /// 创建新的同意管理器
    pub fn new() -> Self {
        let mut manager = Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            purposes: HashMap::new(),
        };
        manager.register_default_purposes();
        manager
    }

    /// 注册默认处理目的
    fn register_default_purposes(&mut self) {
        self.register_purpose(ProcessingPurpose {
            id: "service_delivery".to_string(),
            name: "Service Delivery".to_string(),
            description: "Processing necessary for service delivery".to_string(),
            legal_basis: LegalBasis::ContractPerformance,
            is_required: true,
            retention_days: Some(365),
        });

        self.register_purpose(ProcessingPurpose {
            id: "marketing".to_string(),
            name: "Marketing Communications".to_string(),
            description: "Sending marketing and promotional materials".to_string(),
            legal_basis: LegalBasis::Consent,
            is_required: false,
            retention_days: Some(730),
        });

        self.register_purpose(ProcessingPurpose {
            id: "analytics".to_string(),
            name: "Analytics and Improvement".to_string(),
            description: "Usage analytics and service improvement".to_string(),
            legal_basis: LegalBasis::Consent,
            is_required: false,
            retention_days: Some(365),
        });
    }

    /// 注册处理目的
    pub fn register_purpose(&mut self, purpose: ProcessingPurpose) {
        self.purposes.insert(purpose.id.clone(), purpose);
    }

    /// 获取处理目的
    pub fn get_purpose(&self, purpose_id: &str) -> Option<&ProcessingPurpose> {
        self.purposes.get(purpose_id)
    }

    /// 授予同意
    pub async fn grant_consent(
        &self,
        user_id: &str,
        tenant_id: &str,
        consent_type: ConsentType,
        source: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<ConsentRecord> {
        let record = ConsentRecord::new(user_id, tenant_id, consent_type)
            .grant(source);

        let mut record = record;
        record.ip_address = ip_address;
        record.user_agent = user_agent;

        let mut records = self.records.write().await;
        let user_records = records.entry(user_id.to_string()).or_default();
        user_records.insert(consent_type, record.clone());

        info!(
            "Consent granted for user {} type {:?}",
            user_id, consent_type
        );
        Ok(record)
    }

    /// 撤销同意
    pub async fn withdraw_consent(
        &self,
        user_id: &str,
        consent_type: ConsentType,
    ) -> Result<Option<ConsentRecord>> {
        let mut records = self.records.write().await;

        if let Some(user_records) = records.get_mut(user_id) {
            if let Some(record) = user_records.get_mut(&consent_type) {
                record.withdraw();
                info!(
                    "Consent withdrawn for user {} type {:?}",
                    user_id, consent_type
                );
                return Ok(Some(record.clone()));
            }
        }

        Ok(None)
    }

    /// 检查同意状态
    pub async fn check_consent(
        &self,
        user_id: &str,
        consent_type: ConsentType,
    ) -> Option<ConsentStatus> {
        let records = self.records.read().await;
        records
            .get(user_id)
            .and_then(|ur| ur.get(&consent_type))
            .map(|r| r.status)
    }

    /// 检查是否已授予同意
    pub async fn has_consent(&self, user_id: &str, consent_type: ConsentType) -> bool {
        self.check_consent(user_id, consent_type).await
            == Some(ConsentStatus::Granted)
    }

    /// 获取用户所有同意记录
    pub async fn get_user_consents(&self, user_id: &str) -> Vec<ConsentRecord> {
        let records = self.records.read().await;
        records
            .get(user_id)
            .map(|ur| ur.values().cloned().collect())
            .unwrap_or_default()
    }

    /// 获取用户有效同意类型
    pub async fn get_valid_consents(&self, user_id: &str) -> Vec<ConsentType> {
        let records = self.records.read().await;
        records
            .get(user_id)
            .map(|ur| {
                ur.iter()
                    .filter(|(_, r)| r.is_valid())
                    .map(|(t, _)| *t)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 导出用户同意记录 (用于 GDPR 数据导出)
    pub async fn export_user_consents(&self, user_id: &str) -> serde_json::Value {
        let consents = self.get_user_consents(user_id).await;
        serde_json::to_value(consents).unwrap_or(serde_json::json!([]))
    }
}

impl Default for ConsentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grant_consent() {
        let manager = ConsentManager::new();

        let record = manager
            .grant_consent(
                "user-1",
                "tenant-1",
                ConsentType::Marketing,
                "web",
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(record.status, ConsentStatus::Granted);
        assert!(record.granted_at.is_some());
    }

    #[tokio::test]
    async fn test_withdraw_consent() {
        let manager = ConsentManager::new();

        manager
            .grant_consent(
                "user-1",
                "tenant-1",
                ConsentType::Marketing,
                "web",
                None,
                None,
            )
            .await
            .unwrap();

        let withdrawn = manager
            .withdraw_consent("user-1", ConsentType::Marketing)
            .await
            .unwrap();

        assert!(withdrawn.is_some());
        assert_eq!(withdrawn.unwrap().status, ConsentStatus::Withdrawn);
    }

    #[tokio::test]
    async fn test_check_consent() {
        let manager = ConsentManager::new();

        assert!(!manager.has_consent("user-1", ConsentType::Analytics).await);

        manager
            .grant_consent(
                "user-1",
                "tenant-1",
                ConsentType::Analytics,
                "app",
                None,
                None,
            )
            .await
            .unwrap();

        assert!(manager.has_consent("user-1", ConsentType::Analytics).await);
    }
}
