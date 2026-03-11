//! Data classification for GDPR compliance
//!
//! 实现数据分类框架，支持 4 级敏感度分类

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// 数据敏感度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataSensitivity {
    /// 公开数据 - 无需加密
    Public,
    /// 内部数据 - 可选加密
    Internal,
    /// 机密数据 - 必须加密
    Confidential,
    /// 受限数据 - 加密 + 访问控制
    Restricted,
}

impl Default for DataSensitivity {
    fn default() -> Self {
        Self::Internal
    }
}

impl std::fmt::Display for DataSensitivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "Public"),
            Self::Internal => write!(f, "Internal"),
            Self::Confidential => write!(f, "Confidential"),
            Self::Restricted => write!(f, "Restricted"),
        }
    }
}

/// GDPR 数据类别
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataCategory {
    /// 个人身份信息 (PII)
    PersonalIdentifiableInformation,
    /// 健康数据
    HealthData,
    /// 财务数据
    FinancialData,
    /// 位置数据
    LocationData,
    /// 生物特征数据
    BiometricData,
    /// 种族/民族数据
    EthnicData,
    /// 政治观点
    PoliticalOpinions,
    /// 宗教信仰
    ReligiousBeliefs,
    /// 性取向
    SexualOrientation,
    /// 刑事记录
    CriminalRecords,
    /// 一般业务数据
    GeneralBusiness,
    /// 技术数据 (IP, 设备信息)
    TechnicalData,
    /// 用户生成内容
    UserGeneratedContent,
    /// 其他
    Other(String),
}

impl DataCategory {
    /// 获取数据类别的敏感度级别
    pub fn default_sensitivity(&self) -> DataSensitivity {
        match self {
            // 特殊类别数据 - 最高敏感度
            Self::HealthData
            | Self::BiometricData
            | Self::EthnicData
            | Self::PoliticalOpinions
            | Self::ReligiousBeliefs
            | Self::SexualOrientation
            | Self::CriminalRecords => DataSensitivity::Restricted,

            // 高敏感度数据
            Self::PersonalIdentifiableInformation | Self::FinancialData => {
                DataSensitivity::Confidential
            }

            // 中等敏感度
            Self::LocationData | Self::TechnicalData => DataSensitivity::Internal,

            // 低敏感度
            Self::GeneralBusiness | Self::UserGeneratedContent => DataSensitivity::Internal,

            // 公开数据
            Self::Other(_) => DataSensitivity::Internal,
        }
    }

    /// 是否为 GDPR 特殊类别数据
    pub fn is_special_category(&self) -> bool {
        matches!(
            self,
            Self::HealthData
                | Self::BiometricData
                | Self::EthnicData
                | Self::PoliticalOpinions
                | Self::ReligiousBeliefs
                | Self::SexualOrientation
                | Self::CriminalRecords
        )
    }
}

/// 数据字段分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFieldClassification {
    /// 字段名
    pub field_name: String,
    /// 数据类别
    pub category: DataCategory,
    /// 敏感度
    pub sensitivity: DataSensitivity,
    /// 是否需要加密
    pub requires_encryption: bool,
    /// 是否需要脱敏
    pub requires_masking: bool,
    /// 保留期限 (天)
    pub retention_days: Option<u32>,
    /// 描述
    pub description: Option<String>,
}

impl DataFieldClassification {
    /// 创建新的字段分类
    pub fn new(
        field_name: impl Into<String>,
        category: DataCategory,
        sensitivity: DataSensitivity,
    ) -> Self {
        let requires_encryption = sensitivity >= DataSensitivity::Confidential;
        let requires_masking = sensitivity >= DataSensitivity::Confidential;

        Self {
            field_name: field_name.into(),
            category,
            sensitivity,
            requires_encryption,
            requires_masking,
            retention_days: None,
            description: None,
        }
    }

    /// 设置保留期限
    pub fn with_retention(mut self, days: u32) -> Self {
        self.retention_days = Some(days);
        self
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// 个人数据分类器
pub struct PersonalDataClassifier {
    /// 字段分类映射
    field_classifications: HashMap<String, DataFieldClassification>,
    /// 表级分类
    table_classifications: HashMap<String, DataCategory>,
}

impl PersonalDataClassifier {
    /// 创建新的分类器
    pub fn new() -> Self {
        let mut classifier = Self {
            field_classifications: HashMap::new(),
            table_classifications: HashMap::new(),
        };
        classifier.register_default_classifications();
        classifier
    }

    /// 注册默认分类
    fn register_default_classifications(&mut self) {
        // 用户身份信息
        self.register_field(DataFieldClassification::new(
            "user_id",
            DataCategory::PersonalIdentifiableInformation,
            DataSensitivity::Confidential,
        ));

        self.register_field(DataFieldClassification::new(
            "email",
            DataCategory::PersonalIdentifiableInformation,
            DataSensitivity::Confidential,
        ).with_description("User email address"));

        self.register_field(DataFieldClassification::new(
            "phone",
            DataCategory::PersonalIdentifiableInformation,
            DataSensitivity::Confidential,
        ).with_description("Phone number"));

        self.register_field(DataFieldClassification::new(
            "name",
            DataCategory::PersonalIdentifiableInformation,
            DataSensitivity::Internal,
        ));

        self.register_field(DataFieldClassification::new(
            "ip_address",
            DataCategory::TechnicalData,
            DataSensitivity::Internal,
        ));

        self.register_field(DataFieldClassification::new(
            "device_id",
            DataCategory::TechnicalData,
            DataSensitivity::Internal,
        ));

        // 会话数据
        self.register_field(DataFieldClassification::new(
            "session_id",
            DataCategory::TechnicalData,
            DataSensitivity::Internal,
        ).with_retention(30));

        // 消息内容
        self.register_field(DataFieldClassification::new(
            "message_content",
            DataCategory::UserGeneratedContent,
            DataSensitivity::Confidential,
        ).with_retention(365));

        // 认证令牌
        self.register_field(DataFieldClassification::new(
            "access_token",
            DataCategory::TechnicalData,
            DataSensitivity::Restricted,
        ).with_retention(1));

        self.register_field(DataFieldClassification::new(
            "refresh_token",
            DataCategory::TechnicalData,
            DataSensitivity::Restricted,
        ).with_retention(30));

        info!(
            "Registered {} default field classifications",
            self.field_classifications.len()
        );
    }

    /// 注册字段分类
    pub fn register_field(&mut self, classification: DataFieldClassification) {
        self.field_classifications
            .insert(classification.field_name.clone(), classification);
    }

    /// 获取字段分类
    pub fn get_field_classification(&self, field_name: &str) -> Option<&DataFieldClassification> {
        self.field_classifications.get(field_name)
    }

    /// 获取字段敏感度
    pub fn get_field_sensitivity(&self, field_name: &str) -> DataSensitivity {
        self.field_classifications
            .get(field_name)
            .map(|c| c.sensitivity)
            .unwrap_or_default()
    }

    /// 检查字段是否需要加密
    pub fn requires_encryption(&self, field_name: &str) -> bool {
        self.field_classifications
            .get(field_name)
            .map(|c| c.requires_encryption)
            .unwrap_or(false)
    }

    /// 获取所有受限字段
    pub fn get_restricted_fields(&self) -> Vec<&str> {
        self.field_classifications
            .iter()
            .filter(|(_, c)| c.sensitivity == DataSensitivity::Restricted)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// 获取所有特殊类别字段
    pub fn get_special_category_fields(&self) -> Vec<&str> {
        self.field_classifications
            .iter()
            .filter(|(_, c)| c.category.is_special_category())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// 注册表级分类
    pub fn register_table(&mut self, table_name: impl Into<String>, category: DataCategory) {
        self.table_classifications.insert(table_name.into(), category);
    }

    /// 获取表分类
    pub fn get_table_category(&self, table_name: &str) -> Option<&DataCategory> {
        self.table_classifications.get(table_name)
    }
}

impl Default for PersonalDataClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_sensitivity_ordering() {
        assert!(DataSensitivity::Restricted >= DataSensitivity::Confidential);
        assert!(DataSensitivity::Confidential >= DataSensitivity::Internal);
        assert!(DataSensitivity::Internal >= DataSensitivity::Public);
    }

    #[test]
    fn test_special_category_detection() {
        assert!(DataCategory::HealthData.is_special_category());
        assert!(DataCategory::BiometricData.is_special_category());
        assert!(!DataCategory::TechnicalData.is_special_category());
        assert!(!DataCategory::GeneralBusiness.is_special_category());
    }

    #[test]
    fn test_classifier_default_fields() {
        let classifier = PersonalDataClassifier::new();

        assert!(classifier.requires_encryption("email"));
        assert!(classifier.requires_encryption("access_token"));
        assert!(!classifier.requires_encryption("session_id"));
    }
}
