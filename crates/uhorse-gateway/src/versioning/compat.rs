//! API Compatibility Checker
//!
//! API 兼容性检查和破坏性变更检测

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 兼容性级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityLevel {
    /// 完全兼容
    FullyCompatible,
    /// 向后兼容 (新增功能)
    BackwardCompatible,
    /// 有破坏性变更
    Breaking,
    /// 不兼容
    Incompatible,
}

/// 破坏性变更类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakingChange {
    /// 删除端点
    EndpointRemoved(String),
    /// 重命名端点
    EndpointRenamed { old: String, new: String },
    /// 删除请求字段
    RequestFieldRemoved { endpoint: String, field: String },
    /// 修改请求字段类型
    RequestFieldTypeChanged { endpoint: String, field: String, old_type: String, new_type: String },
    /// 必填字段新增
    RequiredFieldAdded { endpoint: String, field: String },
    /// 响应字段删除
    ResponseFieldRemoved { endpoint: String, field: String },
    /// 响应字段类型修改
    ResponseFieldTypeChanged { endpoint: String, field: String, old_type: String, new_type: String },
    /// 认证方式变更
    AuthenticationChanged { endpoint: String },
    /// 权限要求变更
    PermissionChanged { endpoint: String, old: String, new: String },
}

impl BreakingChange {
    /// 获取变更描述
    pub fn description(&self) -> String {
        match self {
            Self::EndpointRemoved(path) => format!("端点已删除: {}", path),
            Self::EndpointRenamed { old, new } => format!("端点已重命名: {} -> {}", old, new),
            Self::RequestFieldRemoved { endpoint, field } => {
                format!("请求字段已删除: {} / {}", endpoint, field)
            }
            Self::RequestFieldTypeChanged { endpoint, field, old_type, new_type } => {
                format!("请求字段类型已修改: {} / {} ({} -> {})", endpoint, field, old_type, new_type)
            }
            Self::RequiredFieldAdded { endpoint, field } => {
                format!("新增必填字段: {} / {}", endpoint, field)
            }
            Self::ResponseFieldRemoved { endpoint, field } => {
                format!("响应字段已删除: {} / {}", endpoint, field)
            }
            Self::ResponseFieldTypeChanged { endpoint, field, old_type, new_type } => {
                format!("响应字段类型已修改: {} / {} ({} -> {})", endpoint, field, old_type, new_type)
            }
            Self::AuthenticationChanged { endpoint } => {
                format!("认证方式已变更: {}", endpoint)
            }
            Self::PermissionChanged { endpoint, old, new } => {
                format!("权限要求已变更: {} ({} -> {})", endpoint, old, new)
            }
        }
    }

    /// 获取严重程度 (1-5)
    pub fn severity(&self) -> u8 {
        match self {
            Self::EndpointRemoved(_) => 5,
            Self::EndpointRenamed { .. } => 4,
            Self::RequestFieldRemoved { .. } => 4,
            Self::RequestFieldTypeChanged { .. } => 3,
            Self::RequiredFieldAdded { .. } => 3,
            Self::ResponseFieldRemoved { .. } => 3,
            Self::ResponseFieldTypeChanged { .. } => 2,
            Self::AuthenticationChanged { .. } => 5,
            Self::PermissionChanged { .. } => 2,
        }
    }
}

/// API 端点规范
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSpec {
    /// 端点路径
    pub path: String,
    /// HTTP 方法
    pub method: String,
    /// 请求参数
    pub request_params: Vec<FieldSpec>,
    /// 响应字段
    pub response_fields: Vec<FieldSpec>,
    /// 认证要求
    pub auth_required: bool,
    /// 权限要求
    pub permissions: Vec<String>,
}

/// 字段规范
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSpec {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub field_type: String,
    /// 是否必填
    pub required: bool,
    /// 描述
    pub description: Option<String>,
}

/// 兼容性检查器
pub struct CompatibilityChecker {
    /// 旧版本规范
    old_spec: HashMap<String, EndpointSpec>,
    /// 新版本规范
    new_spec: HashMap<String, EndpointSpec>,
}

impl CompatibilityChecker {
    /// 创建新的兼容性检查器
    pub fn new(old_spec: Vec<EndpointSpec>, new_spec: Vec<EndpointSpec>) -> Self {
        let old_map = old_spec.into_iter()
            .map(|s| (format!("{}:{}", s.method, s.path), s))
            .collect();

        let new_map = new_spec.into_iter()
            .map(|s| (format!("{}:{}", s.method, s.path), s))
            .collect();

        Self {
            old_spec: old_map,
            new_spec: new_map,
        }
    }

}

impl CompatibilityChecker {
    /// 检查兼容性
    pub fn check(&self) -> CompatibilityReport {
        let mut breaking_changes = Vec::new();
        let mut warnings = Vec::new();

        // 检查删除的端点
        for (key, old_endpoint) in &self.old_spec {
            if !self.new_spec.contains_key(key) {
                breaking_changes.push(BreakingChange::EndpointRemoved(old_endpoint.path.clone()));
            }
        }

        // 检查修改的端点
        for (key, new_endpoint) in &self.new_spec {
            if let Some(old_endpoint) = self.old_spec.get(key) {
                // 检查请求参数
                for old_field in &old_endpoint.request_params {
                    if !new_endpoint.request_params.iter().any(|f| f.name == old_field.name) {
                        breaking_changes.push(BreakingChange::RequestFieldRemoved {
                            endpoint: new_endpoint.path.clone(),
                            field: old_field.name.clone(),
                        });
                    } else if let Some(new_field) = new_endpoint.request_params.iter().find(|f| f.name == old_field.name) {
                        if new_field.field_type != old_field.field_type {
                            breaking_changes.push(BreakingChange::RequestFieldTypeChanged {
                                endpoint: new_endpoint.path.clone(),
                                field: old_field.name.clone(),
                                old_type: old_field.field_type.clone(),
                                new_type: new_field.field_type.clone(),
                            });
                        }
                        if new_field.required && !old_field.required {
                            breaking_changes.push(BreakingChange::RequiredFieldAdded {
                                endpoint: new_endpoint.path.clone(),
                                field: old_field.name.clone(),
                            });
                        }
                    }
                }

                // 检查响应字段
                for old_field in &old_endpoint.response_fields {
                    if !new_endpoint.response_fields.iter().any(|f| f.name == old_field.name) {
                        breaking_changes.push(BreakingChange::ResponseFieldRemoved {
                            endpoint: new_endpoint.path.clone(),
                            field: old_field.name.clone(),
                        });
                    } else if let Some(new_field) = new_endpoint.response_fields.iter().find(|f| f.name == old_field.name) {
                        if new_field.field_type != old_field.field_type {
                            breaking_changes.push(BreakingChange::ResponseFieldTypeChanged {
                                endpoint: new_endpoint.path.clone(),
                                field: old_field.name.clone(),
                                old_type: old_field.field_type.clone(),
                                new_type: new_field.field_type.clone(),
                            });
                        }
                    }
                }

                // 检查认证变更
                if new_endpoint.auth_required && !old_endpoint.auth_required {
                    breaking_changes.push(BreakingChange::AuthenticationChanged {
                        endpoint: new_endpoint.path.clone(),
                    });
                }

                // 检查权限变更
                for old_perm in &old_endpoint.permissions {
                    if !new_endpoint.permissions.contains(old_perm) {
                        warnings.push(format!(
                            "权限 '{}' 已从端点 {} 移除",
                            old_perm, new_endpoint.path
                        ));
                    }
                }
            }
        }

        let level = if breaking_changes.is_empty() {
            CompatibilityLevel::FullyCompatible
        } else {
            let max_severity = breaking_changes.iter().map(|c| c.severity()).max().unwrap_or(0);
            if max_severity >= 4 {
                CompatibilityLevel::Incompatible
            } else {
                CompatibilityLevel::Breaking
            }
        };

        CompatibilityReport {
            level,
            breaking_changes,
            warnings,
        }
    }
}

/// 兼容性报告
#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    /// 兼容性级别
    pub level: CompatibilityLevel,
    /// 破坏性变更列表
    pub breaking_changes: Vec<BreakingChange>,
    /// 警告列表
    pub warnings: Vec<String>,
}

impl CompatibilityReport {
    /// 是否兼容
    pub fn is_compatible(&self) -> bool {
        matches!(self.level, CompatibilityLevel::FullyCompatible | CompatibilityLevel::BackwardCompatible)
    }

    /// 是否有破坏性变更
    pub fn has_breaking_changes(&self) -> bool {
        !self.breaking_changes.is_empty()
    }

    /// 获取严重程度最高的变更
    pub fn most_severe_change(&self) -> Option<&BreakingChange> {
        self.breaking_changes.iter().max_by_key(|c| c.severity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaking_change_severity() {
        let change = BreakingChange::EndpointRemoved("/api/v1/test".to_string());
        assert_eq!(change.severity(), 5);

        let change = BreakingChange::ResponseFieldTypeChanged {
            endpoint: "/api/v1/test".to_string(),
            field: "id".to_string(),
            old_type: "string".to_string(),
            new_type: "integer".to_string(),
        };
        assert_eq!(change.severity(), 2);
    }

}
