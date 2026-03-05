//! # 审计日志
//!
//! 实现操作日志记录：
//! - 操作日志记录
//! - 日志查询 API
//! - 日志导出

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::rbac::{Action, ResourceType};

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    /// 日志 ID
    pub id: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 用户 ID
    pub user_id: String,
    /// 操作类型
    pub action: Action,
    /// 资源类型
    pub resource_type: ResourceType,
    /// 资源 ID
    pub resource_id: Option<String>,
    /// 操作描述
    pub description: String,
    /// 请求 IP
    pub ip_address: Option<String>,
    /// User Agent
    pub user_agent: Option<String>,
    /// 操作结果
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 附加数据
    pub metadata: Option<serde_json::Value>,
}

impl AuditLog {
    /// 创建新的审计日志
    pub fn new(
        tenant_id: String,
        user_id: String,
        action: Action,
        resource_type: ResourceType,
        description: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id,
            user_id,
            action,
            resource_type,
            resource_id: None,
            description,
            ip_address: None,
            user_agent: None,
            success: true,
            error: None,
            created_at: Utc::now(),
            metadata: None,
        }
    }

    /// 设置资源 ID
    pub fn with_resource_id(mut self, id: impl Into<String>) -> Self {
        self.resource_id = Some(id.into());
        self
    }

    /// 设置 IP 地址
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// 设置 User Agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// 设置操作失败
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.success = false;
        self.error = Some(error.into());
        self
    }

    /// 设置附加数据
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// 审计日志查询参数
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuditQuery {
    /// 租户 ID
    pub tenant_id: Option<String>,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 操作类型
    pub action: Option<Action>,
    /// 资源类型
    pub resource_type: Option<ResourceType>,
    /// 开始时间
    pub start_time: Option<DateTime<Utc>>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 是否成功
    pub success: Option<bool>,
    /// 页码
    pub page: Option<u32>,
    /// 每页数量
    pub page_size: Option<u32>,
}

/// 审计日志管理器
#[derive(Debug)]
pub struct AuditManager {
    /// 内存存储（生产环境应使用数据库）
    logs: Arc<RwLock<VecDeque<AuditLog>>>,
    /// 最大存储数量
    max_logs: usize,
}

impl AuditManager {
    /// 创建审计管理器
    pub fn new(max_logs: usize) -> Self {
        Self {
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(max_logs))),
            max_logs,
        }
    }

    /// 记录审计日志
    pub async fn log(&self, entry: AuditLog) {
        let mut logs = self.logs.write().await;

        // 如果超过最大数量，删除最旧的
        if logs.len() >= self.max_logs {
            logs.pop_front();
        }

        logs.push_back(entry);
    }

    /// 查询审计日志
    pub async fn query(&self, query: AuditQuery) -> Vec<AuditLog> {
        let logs = self.logs.read().await;
        let page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(50);

        logs.iter()
            .filter(|log| {
                // 租户过滤
                if let Some(ref tenant_id) = query.tenant_id {
                    if log.tenant_id != *tenant_id {
                        return false;
                    }
                }

                // 用户过滤
                if let Some(ref user_id) = query.user_id {
                    if log.user_id != *user_id {
                        return false;
                    }
                }

                // 操作类型过滤
                if let Some(action) = query.action {
                    if log.action != action {
                        return false;
                    }
                }

                // 资源类型过滤
                if let Some(resource_type) = query.resource_type {
                    if log.resource_type != resource_type {
                        return false;
                    }
                }

                // 时间范围过滤
                if let Some(start) = query.start_time {
                    if log.created_at < start {
                        return false;
                    }
                }

                if let Some(end) = query.end_time {
                    if log.created_at > end {
                        return false;
                    }
                }

                // 成功状态过滤
                if let Some(success) = query.success {
                    if log.success != success {
                        return false;
                    }
                }

                true
            })
            .skip(((page - 1) * page_size) as usize)
            .take(page_size as usize)
            .cloned()
            .collect()
    }

    /// 获取日志总数
    pub async fn count(&self) -> usize {
        self.logs.read().await.len()
    }

    /// 导出日志（JSON 格式）
    pub async fn export_json(&self, query: AuditQuery) -> String {
        let logs = self.query(query).await;
        serde_json::to_string_pretty(&logs).unwrap_or_default()
    }

    /// 导出日志（CSV 格式）
    pub async fn export_csv(&self, query: AuditQuery) -> String {
        let logs = self.query(query).await;
        let mut csv = String::from("id,tenant_id,user_id,action,resource_type,resource_id,description,success,error,created_at\n");

        for log in logs {
            csv.push_str(&format!(
                "{},{},{},{:?},{:?},{},{},{},{},{}\n",
                log.id,
                log.tenant_id,
                log.user_id,
                log.action,
                log.resource_type,
                log.resource_id.unwrap_or_default(),
                log.description,
                log.success,
                log.error.unwrap_or_default(),
                log.created_at.to_rfc3339()
            ));
        }

        csv
    }
}

impl Default for AuditManager {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_log() {
        let manager = AuditManager::new(100);

        let entry = AuditLog::new(
            "tenant1".to_string(),
            "user1".to_string(),
            Action::Create,
            ResourceType::Agent,
            "创建 Agent".to_string(),
        )
        .with_resource_id("agent1")
        .with_ip("192.168.1.1");

        manager.log(entry).await;

        let logs = manager.query(AuditQuery::default()).await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].user_id, "user1");
    }

    #[tokio::test]
    async fn test_query_filter() {
        let manager = AuditManager::new(100);

        // 添加多条日志
        manager.log(AuditLog::new(
            "tenant1".to_string(),
            "user1".to_string(),
            Action::Create,
            ResourceType::Agent,
            "创建 Agent 1".to_string(),
        )).await;

        manager.log(AuditLog::new(
            "tenant1".to_string(),
            "user2".to_string(),
            Action::Delete,
            ResourceType::Agent,
            "删除 Agent 2".to_string(),
        )).await;

        manager.log(AuditLog::new(
            "tenant2".to_string(),
            "user1".to_string(),
            Action::Create,
            ResourceType::Skill,
            "创建 Skill".to_string(),
        )).await;

        // 按用户过滤
        let logs = manager.query(AuditQuery {
            user_id: Some("user1".to_string()),
            ..Default::default()
        }).await;
        assert_eq!(logs.len(), 2);

        // 按操作类型过滤
        let logs = manager.query(AuditQuery {
            action: Some(Action::Delete),
            ..Default::default()
        }).await;
        assert_eq!(logs.len(), 1);
    }
}
