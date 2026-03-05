//! # RBAC (基于角色的访问控制)
//!
//! 实现企业级权限管理系统：
//! - 角色定义 (admin/operator/viewer)
//! - 资源权限 (Agent/Skill/Channel)
//! - API 鉴权中间件

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// 角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// 管理员 - 完全访问权限
    Admin,
    /// 操作员 - 可创建/修改资源
    Operator,
    /// 观察者 - 只读权限
    Viewer,
}

impl Default for Role {
    fn default() -> Self {
        Self::Viewer
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Operator => write!(f, "operator"),
            Role::Viewer => write!(f, "viewer"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "operator" => Ok(Role::Operator),
            "viewer" => Ok(Role::Viewer),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// 资源类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// Agent
    Agent,
    /// Skill
    Skill,
    /// Session
    Session,
    /// Channel
    Channel,
    /// System
    System,
    /// Tenant
    Tenant,
}

/// 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// 创建
    Create,
    /// 读取
    Read,
    /// 更新
    Update,
    /// 删除
    Delete,
    /// 执行
    Execute,
    /// 管理
    Manage,
}

/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    /// 资源类型
    pub resource: ResourceType,
    /// 允许的操作
    pub actions: HashSet<Action>,
}

impl Permission {
    /// 创建新权限
    pub fn new(resource: ResourceType, actions: Vec<Action>) -> Self {
        Self {
            resource,
            actions: actions.into_iter().collect(),
        }
    }

    /// 检查是否有指定操作权限
    pub fn can(&self, action: Action) -> bool {
        self.actions.contains(&action)
    }
}

/// 用户角色信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRole {
    /// 用户 ID
    pub user_id: String,
    /// 角色
    pub role: Role,
    /// 租户 ID
    pub tenant_id: String,
    /// 自定义权限
    pub custom_permissions: Vec<Permission>,
}

impl UserRole {
    /// 创建新用户角色
    pub fn new(user_id: String, role: Role, tenant_id: String) -> Self {
        Self {
            user_id,
            role,
            tenant_id,
            custom_permissions: Vec::new(),
        }
    }
}

/// RBAC 管理器
#[derive(Debug, Clone)]
pub struct RbacManager {
    /// 角色默认权限
    role_permissions: std::collections::HashMap<Role, Vec<Permission>>,
}

impl RbacManager {
    /// 创建 RBAC 管理器
    pub fn new() -> Self {
        let mut role_permissions = std::collections::HashMap::new();

        // Admin - 完全权限
        role_permissions.insert(
            Role::Admin,
            vec![
                Permission::new(
                    ResourceType::Agent,
                    vec![
                        Action::Create,
                        Action::Read,
                        Action::Update,
                        Action::Delete,
                        Action::Execute,
                        Action::Manage,
                    ],
                ),
                Permission::new(
                    ResourceType::Skill,
                    vec![
                        Action::Create,
                        Action::Read,
                        Action::Update,
                        Action::Delete,
                        Action::Execute,
                    ],
                ),
                Permission::new(
                    ResourceType::Session,
                    vec![Action::Read, Action::Delete, Action::Manage],
                ),
                Permission::new(
                    ResourceType::Channel,
                    vec![
                        Action::Create,
                        Action::Read,
                        Action::Update,
                        Action::Delete,
                        Action::Manage,
                    ],
                ),
                Permission::new(ResourceType::System, vec![Action::Read, Action::Manage]),
                Permission::new(ResourceType::Tenant, vec![Action::Read, Action::Manage]),
            ],
        );

        // Operator - 操作权限
        role_permissions.insert(
            Role::Operator,
            vec![
                Permission::new(
                    ResourceType::Agent,
                    vec![Action::Create, Action::Read, Action::Update, Action::Execute],
                ),
                Permission::new(
                    ResourceType::Skill,
                    vec![Action::Create, Action::Read, Action::Update, Action::Execute],
                ),
                Permission::new(ResourceType::Session, vec![Action::Read]),
                Permission::new(ResourceType::Channel, vec![Action::Read]),
                Permission::new(ResourceType::System, vec![Action::Read]),
            ],
        );

        // Viewer - 只读权限
        role_permissions.insert(
            Role::Viewer,
            vec![
                Permission::new(ResourceType::Agent, vec![Action::Read]),
                Permission::new(ResourceType::Skill, vec![Action::Read]),
                Permission::new(ResourceType::Session, vec![Action::Read]),
                Permission::new(ResourceType::Channel, vec![Action::Read]),
            ],
        );

        Self { role_permissions }
    }

    /// 检查用户是否有权限执行操作
    pub fn check_permission(
        &self,
        user_role: &UserRole,
        resource: ResourceType,
        action: Action,
    ) -> bool {
        // 获取角色默认权限
        if let Some(permissions) = self.role_permissions.get(&user_role.role) {
            for perm in permissions {
                if perm.resource == resource && perm.can(action) {
                    return true;
                }
            }
        }

        // 检查自定义权限
        for perm in &user_role.custom_permissions {
            if perm.resource == resource && perm.can(action) {
                return true;
            }
        }

        false
    }

    /// 获取角色的所有权限
    pub fn get_role_permissions(&self, role: Role) -> Option<&[Permission]> {
        self.role_permissions.get(&role).map(|v| v.as_slice())
    }
}

impl Default for RbacManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_role_permissions() {
        let rbac = RbacManager::new();
        let admin = UserRole::new("user1".to_string(), Role::Admin, "tenant1".to_string());
        let viewer = UserRole::new("user2".to_string(), Role::Viewer, "tenant1".to_string());

        // Admin 可以删除 Agent
        assert!(rbac.check_permission(&admin, ResourceType::Agent, Action::Delete));

        // Viewer 不能删除 Agent
        assert!(!rbac.check_permission(&viewer, ResourceType::Agent, Action::Delete));

        // Viewer 可以读取 Agent
        assert!(rbac.check_permission(&viewer, ResourceType::Agent, Action::Read));
    }

    #[test]
    fn test_custom_permissions() {
        let rbac = RbacManager::new();
        let mut viewer = UserRole::new("user1".to_string(), Role::Viewer, "tenant1".to_string());

        // 添加自定义删除权限
        viewer.custom_permissions.push(Permission::new(
            ResourceType::Agent,
            vec![Action::Delete],
        ));

        // 现在 Viewer 可以删除 Agent
        assert!(rbac.check_permission(&viewer, ResourceType::Agent, Action::Delete));
    }

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::from_str("admin").unwrap(), Role::Admin);
        assert_eq!(Role::from_str("OPERATOR").unwrap(), Role::Operator);
        assert!(Role::from_str("unknown").is_err());
    }
}
