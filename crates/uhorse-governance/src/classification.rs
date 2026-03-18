//! Data classification framework
//!
//! 数据分类框架，定义敏感度级别和自动分类规则

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// 数据敏感度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SensitivityLevel {
    /// 公开 - 可公开访问
    Public = 0,
    /// 内部 - 仅限内部使用
    Internal = 1,
    /// 机密 - 需要访问控制
    Confidential = 2,
    /// 受限 - 需要特殊授权
    Restricted = 3,
}

impl Default for SensitivityLevel {
    fn default() -> Self {
        Self::Internal
    }
}

impl std::fmt::Display for SensitivityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "Public"),
            Self::Internal => write!(f, "Internal"),
            Self::Confidential => write!(f, "Confidential"),
            Self::Restricted => write!(f, "Restricted"),
        }
    }
}

/// 数据类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// 个人身份信息 (PII)
    PersonalIdentifiableInfo,
    /// 财务数据
    Financial,
    /// 健康数据
    Health,
    /// 位置数据
    Location,
    /// 行为数据
    Behavioral,
    /// 技术日志
    TechnicalLogs,
    /// 配置数据
    Configuration,
    /// 用户生成内容
    UserGenerated,
    /// 系统元数据
    SystemMetadata,
    /// 其他
    Other(String),
}

impl DataType {
    /// 获取默认敏感度
    pub fn default_sensitivity(&self) -> SensitivityLevel {
        match self {
            Self::PersonalIdentifiableInfo | Self::Health => SensitivityLevel::Restricted,
            Self::Financial | Self::Location | Self::Behavioral => SensitivityLevel::Confidential,
            Self::TechnicalLogs | Self::UserGenerated => SensitivityLevel::Internal,
            Self::Configuration | Self::SystemMetadata => SensitivityLevel::Internal,
            Self::Other(_) => SensitivityLevel::Internal,
        }
    }

    /// 是否需要加密
    pub fn requires_encryption(&self) -> bool {
        self.default_sensitivity() >= SensitivityLevel::Confidential
    }

    /// 是否需要脱敏
    pub fn requires_masking(&self) -> bool {
        matches!(self, Self::PersonalIdentifiableInfo | Self::Health)
    }
}

/// 数据分类规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 敏感度级别
    pub sensitivity: SensitivityLevel,
    /// 匹配模式 (正则表达式)
    pub pattern: Option<String>,
    /// 字段名称匹配
    pub field_names: Vec<String>,
    /// 描述
    pub description: Option<String>,
    /// 是否启用
    pub enabled: bool,
}

impl ClassificationRule {
    /// 创建新规则
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        data_type: DataType,
        sensitivity: SensitivityLevel,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data_type,
            sensitivity,
            pattern: None,
            field_names: Vec::new(),
            description: None,
            enabled: true,
        }
    }

    /// 添加字段名匹配
    pub fn with_field_names(mut self, names: Vec<&str>) -> Self {
        self.field_names = names.iter().map(|s| s.to_string()).collect();
        self
    }

    /// 添加正则匹配
    pub fn with_pattern(mut self, pattern: &str) -> Self {
        self.pattern = Some(pattern.to_string());
        self
    }
}

/// 数据分类结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// 数据类型
    pub data_type: DataType,
    /// 敏感度级别
    pub sensitivity: SensitivityLevel,
    /// 匹配的规则 ID
    pub matched_rule: Option<String>,
    /// 是否需要加密
    pub requires_encryption: bool,
    /// 是否需要脱敏
    pub requires_masking: bool,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
}

/// 数据分类器
pub struct DataClassifier {
    /// 分类规则
    rules: Vec<ClassificationRule>,
    /// 字段名到规则的映射
    field_map: HashMap<String, usize>,
}

impl DataClassifier {
    /// 创建新的分类器
    pub fn new() -> Self {
        let mut classifier = Self {
            rules: Vec::new(),
            field_map: HashMap::new(),
        };
        classifier.register_default_rules();
        classifier
    }

    /// 注册默认分类规则
    fn register_default_rules(&mut self) {
        // PII 规则
        self.register_rule(
            ClassificationRule::new(
                "pii-email",
                "Email Address",
                DataType::PersonalIdentifiableInfo,
                SensitivityLevel::Confidential,
            )
            .with_field_names(vec!["email", "email_address", "mail"]),
        );

        self.register_rule(
            ClassificationRule::new(
                "pii-phone",
                "Phone Number",
                DataType::PersonalIdentifiableInfo,
                SensitivityLevel::Confidential,
            )
            .with_field_names(vec!["phone", "phone_number", "mobile", "telephone"]),
        );

        self.register_rule(
            ClassificationRule::new(
                "pii-name",
                "Person Name",
                DataType::PersonalIdentifiableInfo,
                SensitivityLevel::Confidential,
            )
            .with_field_names(vec!["name", "first_name", "last_name", "full_name"]),
        );

        // 财务数据规则
        self.register_rule(
            ClassificationRule::new(
                "financial-card",
                "Credit Card",
                DataType::Financial,
                SensitivityLevel::Restricted,
            )
            .with_field_names(vec!["card_number", "credit_card", "card"]),
        );

        self.register_rule(
            ClassificationRule::new(
                "financial-account",
                "Bank Account",
                DataType::Financial,
                SensitivityLevel::Restricted,
            )
            .with_field_names(vec!["account_number", "bank_account", "iban"]),
        );

        // 技术数据规则
        self.register_rule(
            ClassificationRule::new(
                "tech-ip",
                "IP Address",
                DataType::TechnicalLogs,
                SensitivityLevel::Internal,
            )
            .with_field_names(vec!["ip", "ip_address", "client_ip"]),
        );

        self.register_rule(
            ClassificationRule::new(
                "tech-token",
                "Access Token",
                DataType::TechnicalLogs,
                SensitivityLevel::Confidential,
            )
            .with_field_names(vec!["token", "access_token", "api_key", "secret"]),
        );

        info!(
            "Registered {} classification rules with {} field mappings",
            self.rules.len(),
            self.field_map.len()
        );
    }

    /// 注册分类规则
    pub fn register_rule(&mut self, rule: ClassificationRule) {
        let rule_idx = self.rules.len();

        // 建立字段名映射
        for field_name in &rule.field_names {
            self.field_map.insert(field_name.clone(), rule_idx);
        }

        self.rules.push(rule);
    }

    /// 根据字段名分类
    pub fn classify_by_field(&self, field_name: &str) -> Option<ClassificationResult> {
        let normalized = field_name.to_lowercase().replace('_', "");

        // 尝试精确匹配
        if let Some(&idx) = self.field_map.get(field_name) {
            let rule = &self.rules[idx];
            return Some(ClassificationResult {
                data_type: rule.data_type.clone(),
                sensitivity: rule.sensitivity,
                matched_rule: Some(rule.id.clone()),
                requires_encryption: rule.data_type.requires_encryption(),
                requires_masking: rule.data_type.requires_masking(),
                confidence: 1.0,
            });
        }

        // 尝试规范化匹配
        for (mapped_field, &idx) in &self.field_map {
            let mapped_normalized = mapped_field.to_lowercase().replace('_', "");
            if normalized == mapped_normalized {
                let rule = &self.rules[idx];
                return Some(ClassificationResult {
                    data_type: rule.data_type.clone(),
                    sensitivity: rule.sensitivity,
                    matched_rule: Some(rule.id.clone()),
                    requires_encryption: rule.data_type.requires_encryption(),
                    requires_masking: rule.data_type.requires_masking(),
                    confidence: 0.9,
                });
            }
        }

        // 模糊匹配
        for (mapped_field, &idx) in &self.field_map {
            if normalized.contains(&mapped_field.to_lowercase().replace('_', ""))
                || mapped_field
                    .to_lowercase()
                    .replace('_', "")
                    .contains(&normalized)
            {
                let rule = &self.rules[idx];
                return Some(ClassificationResult {
                    data_type: rule.data_type.clone(),
                    sensitivity: rule.sensitivity,
                    matched_rule: Some(rule.id.clone()),
                    requires_encryption: rule.data_type.requires_encryption(),
                    requires_masking: rule.data_type.requires_masking(),
                    confidence: 0.7,
                });
            }
        }

        None
    }

    /// 批量分类字段
    pub fn classify_fields(&self, fields: &[&str]) -> HashMap<String, ClassificationResult> {
        let mut results = HashMap::new();

        for field in fields {
            if let Some(result) = self.classify_by_field(field) {
                results.insert(field.to_string(), result);
            } else {
                // 默认分类
                results.insert(
                    field.to_string(),
                    ClassificationResult {
                        data_type: DataType::Other("unknown".to_string()),
                        sensitivity: SensitivityLevel::Internal,
                        matched_rule: None,
                        requires_encryption: false,
                        requires_masking: false,
                        confidence: 0.5,
                    },
                );
            }
        }

        results
    }

    /// 获取所有规则
    pub fn get_rules(&self) -> &[ClassificationRule] {
        &self.rules
    }

    /// 获取指定敏感度级别的字段
    pub fn get_fields_by_sensitivity(&self, level: SensitivityLevel) -> Vec<&str> {
        self.rules
            .iter()
            .filter(|r| r.sensitivity == level)
            .flat_map(|r| r.field_names.iter().map(String::as_str))
            .collect()
    }
}

impl Default for DataClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitivity_ordering() {
        assert!(SensitivityLevel::Restricted > SensitivityLevel::Confidential);
        assert!(SensitivityLevel::Confidential > SensitivityLevel::Internal);
        assert!(SensitivityLevel::Internal > SensitivityLevel::Public);
    }

    #[test]
    fn test_data_type_encryption() {
        assert!(DataType::PersonalIdentifiableInfo.requires_encryption());
        assert!(DataType::Health.requires_encryption());
        assert!(!DataType::Configuration.requires_encryption());
    }

    #[test]
    fn test_classify_field() {
        let classifier = DataClassifier::new();

        let result = classifier.classify_by_field("email").unwrap();
        assert_eq!(result.sensitivity, SensitivityLevel::Confidential);
        assert!(result.requires_encryption);
    }

    #[test]
    fn test_classify_unknown_field() {
        let classifier = DataClassifier::new();

        let result = classifier.classify_by_field("custom_field");
        assert!(result.is_none());
    }
}
