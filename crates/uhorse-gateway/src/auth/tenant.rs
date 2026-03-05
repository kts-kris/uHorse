//! # 多租户架构
//!
//! 实现租户隔离和资源配额：
//! - 租户隔离 (TenantId 贯穿所有资源)
//! - 资源配额 (Agent 数量、消息量、存储空间限制)
//! - 使用量统计

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 租户 ID
pub type TenantId = String;

/// 租户计划
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantPlan {
    /// 免费版
    Free,
    /// 专业版
    Pro,
    /// 企业版
    Enterprise,
}

impl Default for TenantPlan {
    fn default() -> Self {
        Self::Free
    }
}

/// 资源配额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// 最大 Agent 数量
    pub max_agents: u32,
    /// 最大技能数量
    pub max_skills: u32,
    /// 每日消息限制
    pub messages_per_day: u32,
    /// 存储空间限制 (字节)
    pub storage_bytes: u64,
    /// API 请求限制 (每分钟)
    pub requests_per_minute: u32,
}

impl ResourceQuota {
    /// 获取计划对应的配额
    pub fn for_plan(plan: TenantPlan) -> Self {
        match plan {
            TenantPlan::Free => Self {
                max_agents: 3,
                max_skills: 10,
                messages_per_day: 100,
                storage_bytes: 100 * 1024 * 1024, // 100MB
                requests_per_minute: 30,
            },
            TenantPlan::Pro => Self {
                max_agents: 20,
                max_skills: 100,
                messages_per_day: 10000,
                storage_bytes: 10 * 1024 * 1024 * 1024, // 10GB
                requests_per_minute: 300,
            },
            TenantPlan::Enterprise => Self {
                max_agents: 1000,
                max_skills: 10000,
                messages_per_day: u32::MAX,
                storage_bytes: u64::MAX,
                requests_per_minute: 10000,
            },
        }
    }
}

/// 租户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    /// 租户 ID
    pub id: TenantId,
    /// 租户名称
    pub name: String,
    /// 计划
    pub plan: TenantPlan,
    /// 资源配额
    pub quota: ResourceQuota,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 联系邮箱
    pub contact_email: Option<String>,
    /// 自定义配置
    pub config: Option<serde_json::Value>,
}

impl Tenant {
    /// 创建新租户
    pub fn new(name: String, plan: TenantPlan) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            plan,
            quota: ResourceQuota::for_plan(plan),
            enabled: true,
            created_at: now,
            updated_at: now,
            contact_email: None,
            config: None,
        }
    }
}

/// 使用量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// 租户 ID
    pub tenant_id: TenantId,
    /// 日期 (YYYY-MM-DD)
    pub date: String,
    /// Agent 数量
    pub agents_count: u32,
    /// 技能数量
    pub skills_count: u32,
    /// 消息数量
    pub messages_count: u64,
    /// API 请求数
    pub requests_count: u64,
    /// 存储使用量 (字节)
    pub storage_used: u64,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl UsageRecord {
    /// 创建新的使用量记录
    pub fn new(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            date: Utc::now().format("%Y-%m-%d").to_string(),
            agents_count: 0,
            skills_count: 0,
            messages_count: 0,
            requests_count: 0,
            storage_used: 0,
            updated_at: Utc::now(),
        }
    }
}

/// 配额检查结果
#[derive(Debug, Clone)]
pub enum QuotaCheck {
    /// 通过
    Allowed,
    /// 超出配额
    Exceeded {
        /// 资源类型
        resource: String,
        /// 当前使用量
        current: u64,
        /// 配额限制
        limit: u64,
    },
}

/// 租户管理器
#[derive(Debug)]
pub struct TenantManager {
    /// 租户存储
    tenants: Arc<RwLock<HashMap<TenantId, Tenant>>>,
    /// 使用量存储
    usage: Arc<RwLock<HashMap<TenantId, UsageRecord>>>,
}

impl TenantManager {
    /// 创建租户管理器
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            usage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建租户
    pub async fn create_tenant(&self, name: String, plan: TenantPlan) -> Tenant {
        let tenant = Tenant::new(name, plan);
        let tenant_id = tenant.id.clone();

        self.tenants
            .write()
            .await
            .insert(tenant_id.clone(), tenant.clone());
        self.usage
            .write()
            .await
            .insert(tenant_id, UsageRecord::new(tenant.id.clone()));

        tenant
    }

    /// 获取租户
    pub async fn get_tenant(&self, id: &str) -> Option<Tenant> {
        self.tenants.read().await.get(id).cloned()
    }

    /// 更新租户计划
    pub async fn update_plan(&self, id: &str, plan: TenantPlan) -> Option<Tenant> {
        let mut tenants = self.tenants.write().await;
        if let Some(tenant) = tenants.get_mut(id) {
            tenant.plan = plan;
            tenant.quota = ResourceQuota::for_plan(plan);
            tenant.updated_at = Utc::now();
            return Some(tenant.clone());
        }
        None
    }

    /// 检查配额
    pub async fn check_quota(&self, tenant_id: &str, resource: &str) -> QuotaCheck {
        let tenant = match self.get_tenant(tenant_id).await {
            Some(t) => t,
            None => {
                return QuotaCheck::Exceeded {
                    resource: "tenant".to_string(),
                    current: 0,
                    limit: 0,
                }
            }
        };

        let usage = self.usage.read().await;
        let record = usage.get(tenant_id);

        match resource {
            "agents" => {
                let current = record.map(|r| r.agents_count as u64).unwrap_or(0);
                if current >= tenant.quota.max_agents as u64 {
                    return QuotaCheck::Exceeded {
                        resource: "agents".to_string(),
                        current,
                        limit: tenant.quota.max_agents as u64,
                    };
                }
            }
            "skills" => {
                let current = record.map(|r| r.skills_count as u64).unwrap_or(0);
                if current >= tenant.quota.max_skills as u64 {
                    return QuotaCheck::Exceeded {
                        resource: "skills".to_string(),
                        current,
                        limit: tenant.quota.max_skills as u64,
                    };
                }
            }
            "messages" => {
                let current = record.map(|r| r.messages_count).unwrap_or(0);
                if current >= tenant.quota.messages_per_day as u64 {
                    return QuotaCheck::Exceeded {
                        resource: "messages".to_string(),
                        current,
                        limit: tenant.quota.messages_per_day as u64,
                    };
                }
            }
            "storage" => {
                let current = record.map(|r| r.storage_used).unwrap_or(0);
                if current >= tenant.quota.storage_bytes {
                    return QuotaCheck::Exceeded {
                        resource: "storage".to_string(),
                        current,
                        limit: tenant.quota.storage_bytes,
                    };
                }
            }
            _ => {}
        }

        QuotaCheck::Allowed
    }

    /// 增加消息计数
    pub async fn increment_messages(&self, tenant_id: &str, count: u64) {
        let mut usage = self.usage.write().await;
        let record = usage
            .entry(tenant_id.to_string())
            .or_insert_with(|| UsageRecord::new(tenant_id.to_string()));
        record.messages_count += count;
        record.updated_at = Utc::now();
    }

    /// 增加请求计数
    pub async fn increment_requests(&self, tenant_id: &str, count: u64) {
        let mut usage = self.usage.write().await;
        let record = usage
            .entry(tenant_id.to_string())
            .or_insert_with(|| UsageRecord::new(tenant_id.to_string()));
        record.requests_count += count;
        record.updated_at = Utc::now();
    }

    /// 获取使用量
    pub async fn get_usage(&self, tenant_id: &str) -> Option<UsageRecord> {
        self.usage.read().await.get(tenant_id).cloned()
    }

    /// 列出所有租户
    pub async fn list_tenants(&self) -> Vec<Tenant> {
        self.tenants.read().await.values().cloned().collect()
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_tenant() {
        let manager = TenantManager::new();
        let tenant = manager
            .create_tenant("Test Company".to_string(), TenantPlan::Pro)
            .await;

        assert_eq!(tenant.name, "Test Company");
        assert_eq!(tenant.plan, TenantPlan::Pro);
        assert!(tenant.enabled);

        let fetched = manager.get_tenant(&tenant.id).await;
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_quota_check() {
        let manager = TenantManager::new();
        let tenant = manager
            .create_tenant("Test".to_string(), TenantPlan::Free)
            .await;

        // 免费版最多 3 个 Agent
        let check = manager.check_quota(&tenant.id, "agents").await;
        assert!(matches!(check, QuotaCheck::Allowed));
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let manager = TenantManager::new();
        let tenant = manager
            .create_tenant("Test".to_string(), TenantPlan::Pro)
            .await;

        manager.increment_messages(&tenant.id, 100).await;
        manager.increment_requests(&tenant.id, 50).await;

        let usage = manager.get_usage(&tenant.id).await.unwrap();
        assert_eq!(usage.messages_count, 100);
        assert_eq!(usage.requests_count, 50);
    }

    #[test]
    fn test_quota_for_plan() {
        let free = ResourceQuota::for_plan(TenantPlan::Free);
        assert_eq!(free.max_agents, 3);

        let pro = ResourceQuota::for_plan(TenantPlan::Pro);
        assert_eq!(pro.max_agents, 20);

        let enterprise = ResourceQuota::for_plan(TenantPlan::Enterprise);
        assert_eq!(enterprise.max_agents, 1000);
    }
}
