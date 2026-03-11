//! Data retention policy
//!
//! 数据保留策略，定义数据生命周期和自动过期

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::classification::{DataType, SensitivityLevel};

/// 保留动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionAction {
    /// 删除数据
    Delete,
    /// 归档数据
    Archive,
    /// 脱敏数据
    Anonymize,
    /// 通知管理员
    NotifyAdmin,
}

/// 保留策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// 策略 ID
    pub id: String,
    /// 策略名称
    pub name: String,
    /// 数据类型 (可选，None 表示所有类型)
    pub data_type: Option<DataType>,
    /// 敏感度级别 (可选，None 表示所有级别)
    pub sensitivity: Option<SensitivityLevel>,
    /// 保留天数
    pub retention_days: u32,
    /// 到期动作
    pub action: RetentionAction,
    /// 是否启用
    pub enabled: bool,
    /// 描述
    pub description: Option<String>,
}

impl RetentionPolicy {
    /// 创建新的保留策略
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        retention_days: u32,
        action: RetentionAction,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data_type: None,
            sensitivity: None,
            retention_days,
            action,
            enabled: true,
            description: None,
        }
    }

    /// 设置数据类型
    pub fn for_data_type(mut self, data_type: DataType) -> Self {
        self.data_type = Some(data_type);
        self
    }

    /// 设置敏感度级别
    pub fn for_sensitivity(mut self, sensitivity: SensitivityLevel) -> Self {
        self.sensitivity = Some(sensitivity);
        self
    }

    /// 检查是否适用于指定数据
    pub fn applies_to(&self, data_type: &DataType, sensitivity: SensitivityLevel) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(ref dt) = self.data_type {
            if dt != data_type {
                return false;
            }
        }

        if let Some(ref s) = self.sensitivity {
            if *s != sensitivity {
                return false;
            }
        }

        true
    }

    /// 计算过期时间
    pub fn calculate_expiry(&self, created_at: DateTime<Utc>) -> DateTime<Utc> {
        created_at + chrono::Duration::days(self.retention_days as i64)
    }

    /// 检查数据是否已过期
    pub fn is_expired(&self, created_at: DateTime<Utc>) -> bool {
        let expiry = self.calculate_expiry(created_at);
        Utc::now() > expiry
    }
}

/// 数据记录元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// 记录 ID
    pub id: String,
    /// 数据类型
    pub data_type: DataType,
    /// 敏感度级别
    pub sensitivity: SensitivityLevel,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后访问时间
    pub last_accessed: Option<DateTime<Utc>>,
    /// 大小 (字节)
    pub size_bytes: u64,
    /// 存储位置
    pub location: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
}

impl DataRecord {
    /// 创建新记录
    pub fn new(
        id: impl Into<String>,
        data_type: DataType,
        sensitivity: SensitivityLevel,
        location: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            data_type,
            sensitivity,
            created_at: Utc::now(),
            last_accessed: None,
            size_bytes: 0,
            location: location.into(),
            tenant_id: tenant_id.into(),
            metadata: HashMap::new(),
        }
    }

    /// 设置大小
    pub fn with_size(mut self, size: u64) -> Self {
        self.size_bytes = size;
        self
    }
}

/// 保留检查结果
#[derive(Debug, Clone)]
pub struct RetentionCheckResult {
    /// 记录 ID
    pub record_id: String,
    /// 匹配的策略 ID
    pub policy_id: String,
    /// 是否已过期
    pub is_expired: bool,
    /// 建议动作
    pub suggested_action: RetentionAction,
    /// 过期时间
    pub expiry_date: DateTime<Utc>,
}

/// 保留策略管理器
pub struct RetentionPolicyManager {
    /// 保留策略列表
    policies: Arc<RwLock<Vec<RetentionPolicy>>>,
}

impl RetentionPolicyManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        let policies = Self::get_default_policies();
        info!("Registered {} default retention policies", policies.len());
        Self {
            policies: Arc::new(RwLock::new(policies)),
        }
    }

    /// 获取默认保留策略
    fn get_default_policies() -> Vec<RetentionPolicy> {
        vec![
            // PII 数据保留 90 天后删除
            RetentionPolicy::new(
                "pii-retention",
                "PII Data Retention",
                90,
                RetentionAction::Delete,
            )
            .for_sensitivity(SensitivityLevel::Restricted)
            .for_data_type(DataType::PersonalIdentifiableInfo),
            // 财务数据保留 7 年 (2555 天)
            RetentionPolicy::new(
                "financial-retention",
                "Financial Data Retention",
                2555, // 7 years
                RetentionAction::Archive,
            )
            .for_data_type(DataType::Financial),
            // 技术日志保留 30 天
            RetentionPolicy::new(
                "logs-retention",
                "Technical Logs Retention",
                30,
                RetentionAction::Delete,
            )
            .for_data_type(DataType::TechnicalLogs),
            // 会话数据保留 1 年
            RetentionPolicy::new(
                "session-retention",
                "Session Data Retention",
                365,
                RetentionAction::Anonymize,
            )
            .for_data_type(DataType::Behavioral),
            // 系统元数据保留 90 天
            RetentionPolicy::new(
                "metadata-retention",
                "System Metadata Retention",
                90,
                RetentionAction::Delete,
            )
            .for_data_type(DataType::SystemMetadata),
        ]
    }

    /// 注册保留策略
    pub async fn register_policy(&self, policy: RetentionPolicy) {
        let mut policies = self.policies.write().await;
        policies.push(policy);
    }

    /// 获取所有策略
    pub async fn get_policies(&self) -> Vec<RetentionPolicy> {
        let policies = self.policies.read().await;
        policies.clone()
    }

    /// 查找适用于数据的策略
    pub async fn find_applicable_policy(
        &self,
        data_type: &DataType,
        sensitivity: SensitivityLevel,
    ) -> Option<RetentionPolicy> {
        let policies = self.policies.read().await;

        // 优先匹配最具体的策略
        for policy in policies.iter() {
            if policy.applies_to(data_type, sensitivity) {
                return Some(policy.clone());
            }
        }

        None
    }

    /// 检查数据记录的保留状态
    pub async fn check_retention(&self, record: &DataRecord) -> Option<RetentionCheckResult> {
        let policy = self.find_applicable_policy(&record.data_type, record.sensitivity).await?;

        Some(RetentionCheckResult {
            record_id: record.id.clone(),
            policy_id: policy.id.clone(),
            is_expired: policy.is_expired(record.created_at),
            suggested_action: policy.action,
            expiry_date: policy.calculate_expiry(record.created_at),
        })
    }

    /// 批量检查保留状态
    pub async fn batch_check(&self, records: &[DataRecord]) -> Vec<RetentionCheckResult> {
        let mut results = Vec::new();

        for record in records {
            if let Some(result) = self.check_retention(record).await {
                results.push(result);
            }
        }

        results
    }

    /// 获取已过期记录
    pub async fn get_expired_records(&self, records: &[DataRecord]) -> Vec<RetentionCheckResult> {
        let checks = self.batch_check(records).await;
        checks.into_iter().filter(|c| c.is_expired).collect()
    }

    /// 获取即将过期的记录 (指定天数内)
    pub async fn get_expiring_soon(
        &self,
        records: &[DataRecord],
        days_threshold: u32,
    ) -> Vec<RetentionCheckResult> {
        let checks = self.batch_check(records).await;
        let threshold = chrono::Duration::days(days_threshold as i64);

        checks
            .into_iter()
            .filter(|c| {
                !c.is_expired && (c.expiry_date - Utc::now()) < threshold
            })
            .collect()
    }
}

impl Default for RetentionPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy_creation() {
        let policy = RetentionPolicy::new(
            "test-policy",
            "Test Policy",
            30,
            RetentionAction::Delete,
        );

        assert_eq!(policy.retention_days, 30);
        assert!(policy.enabled);
    }

    #[test]
    fn test_expiry_calculation() {
        let policy = RetentionPolicy::new(
            "test-policy",
            "Test Policy",
            30,
            RetentionAction::Delete,
        );

        let created = Utc::now() - chrono::Duration::days(31);
        assert!(policy.is_expired(created));

        let recent = Utc::now() - chrono::Duration::days(15);
        assert!(!policy.is_expired(recent));
    }

    #[tokio::test]
    async fn test_policy_manager() {
        let manager = RetentionPolicyManager::new();
        let policies = manager.get_policies().await;

        assert!(!policies.is_empty());
    }

    #[tokio::test]
    async fn test_find_applicable_policy() {
        let manager = RetentionPolicyManager::new();

        let policy = manager
            .find_applicable_policy(
                &DataType::TechnicalLogs,
                SensitivityLevel::Internal,
            )
            .await;

        assert!(policy.is_some());
        assert_eq!(policy.unwrap().retention_days, 30);
    }
}
