//! Data erasure for GDPR compliance
//!
//! 数据删除功能 (GDPR Article 17: 被遗忘权)

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::Result;

/// 删除请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureRequest {
    /// 请求 ID
    pub id: Uuid,
    /// 用户 ID
    pub user_id: String,
    /// 租户 ID
    pub tenant_id: String,
    /// 请求类型
    pub request_type: ErasureType,
    /// 请求的数据类别
    pub data_categories: Vec<String>,
    /// 创建时间
    pub created_at: u64,
    /// 状态
    pub status: ErasureStatus,
    /// 完成时间
    pub completed_at: Option<u64>,
    /// 验证状态
    pub verification: Option<ErasureVerification>,
    /// 备注信息
    pub notes: Option<String>,
}

impl ErasureRequest {
    /// 创建新的删除请求
    pub fn new(
        user_id: impl Into<String>,
        tenant_id: impl Into<String>,
        request_type: ErasureType,
        data_categories: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;

        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            tenant_id: tenant_id.into(),
            request_type,
            data_categories,
            created_at: now,
            status: ErasureStatus::Pending,
            completed_at: None,
            verification: None,
            notes: None,
        }
    }
}

/// 删除类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErasureType {
    /// 完全删除 (所有数据)
    FullErasure,
    /// 部分删除 (指定类别)
    PartialErasure,
    /// 匿名化 (保留统计数据)
    Anonymization,
    /// 软删除 (标记为已删除)
    SoftDelete,
}

/// 删除状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErasureStatus {
    /// 待处理
    Pending,
    /// 验证中
    Verifying,
    /// 处理中
    Processing,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 拒绝 (法律义务)
    Rejected,
}

/// 删除验证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureVerification {
    /// 验证 ID
    pub id: Uuid,
    /// 关联的删除请求 ID
    pub erasure_request_id: Uuid,
    /// 验证方法
    pub method: VerificationMethod,
    /// 验证时间
    pub verified_at: u64,
    /// 验证结果
    pub result: VerificationResult,
    /// 详细信息
    pub details: HashMap<String, String>,
}

/// 验证方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationMethod {
    /// 数据库查询
    DatabaseQuery,
    /// 存储扫描
    StorageScan,
    /// 日志检查
    LogInspection,
    /// 备份验证
    BackupVerification,
    /// 第三方审计
    ThirdPartyAudit,
}

/// 验证结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationResult {
    /// 验证通过
    Verified,
    /// 部分验证
    PartiallyVerified,
    /// 验证失败
    Failed,
    /// 待验证
    Pending,
}

/// 数据删除管理器
pub struct DataErasureManager {
    /// 删除请求存储
    requests: Arc<RwLock<HashMap<Uuid, ErasureRequest>>>,
    /// 数据源处理器
    data_handlers: HashMap<String, Box<dyn DataHandler>>,
}

/// 数据处理器 trait
#[async_trait::async_trait]
pub trait DataHandler: Send + Sync {
    /// 获取处理器名称
    fn name(&self) -> &str;

    /// 删除用户数据
    async fn erase_data(
        &self,
        user_id: &str,
        tenant_id: &str,
        categories: &[String],
    ) -> Result<ErasureResult>;

    /// 验证数据已删除
    async fn verify_erasure(
        &self,
        user_id: &str,
        tenant_id: &str,
        categories: &[String],
    ) -> Result<VerificationResult>;
}

/// 删除结果
#[derive(Debug, Clone)]
pub struct ErasureResult {
    /// 处理器名称
    pub handler: String,
    /// 删除的记录数
    pub records_deleted: u64,
    /// 删除的文件数
    pub files_deleted: u64,
    /// 释放的空间 (字节)
    pub bytes_freed: u64,
    /// 跳过的项目
    pub skipped: Vec<String>,
    /// 错误信息
    pub errors: Vec<String>,
}

impl Default for ErasureResult {
    fn default() -> Self {
        Self {
            handler: String::new(),
            records_deleted: 0,
            files_deleted: 0,
            bytes_freed: 0,
            skipped: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl DataErasureManager {
    /// 创建新的删除管理器
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            data_handlers: HashMap::new(),
        }
    }

    /// 注册数据处理器
    pub fn register_handler(&mut self, handler: Box<dyn DataHandler>) {
        let name = handler.name().to_string();
        self.data_handlers.insert(name, handler);
    }

    /// 创建删除请求
    pub async fn create_request(
        &self,
        user_id: &str,
        tenant_id: &str,
        request_type: ErasureType,
        data_categories: Vec<String>,
    ) -> Result<ErasureRequest> {
        let request = ErasureRequest::new(user_id, tenant_id, request_type, data_categories);

        let mut requests = self.requests.write().await;
        requests.insert(request.id, request.clone());

        info!(
            "Created erasure request {} for user {}",
            request.id, user_id
        );
        Ok(request)
    }

    /// 执行删除请求
    pub async fn execute_erasure(&self, request_id: &Uuid) -> Result<ErasureRequest> {
        let mut requests = self.requests.write().await;

        let request = requests
            .get_mut(request_id)
            .ok_or_else(|| super::GdprError::UserNotFound(request_id.to_string()))?;

        if request.status != ErasureStatus::Pending {
            return Err(super::GdprError::ErasureFailed(format!(
                "Request already {}",
                serde_json::to_string(&request.status).unwrap_or_default()
            )));
        }

        request.status = ErasureStatus::Processing;
        let mut all_results = Vec::new();

        for handler in self.data_handlers.values() {
            debug!("Processing erasure with handler: {}", handler.name());

            match handler
                .erase_data(
                    &request.user_id,
                    &request.tenant_id,
                    &request.data_categories,
                )
                .await
            {
                Ok(result) => {
                    all_results.push(result);
                }
                Err(e) => {
                    warn!("Erasure failed for handler {}: {}", handler.name(), e);
                }
            }
        }

        // 验证删除
        request.status = ErasureStatus::Verifying;
        let mut verification_results = Vec::new();

        for handler in self.data_handlers.values() {
            if let Ok(result) = handler
                .verify_erasure(
                    &request.user_id,
                    &request.tenant_id,
                    &request.data_categories,
                )
                .await
            {
                verification_results.push(result);
            }
        }

        // 确定最终状态
        let all_verified = verification_results.iter().all(|r| *r == VerificationResult::Verified);
        let any_failed = verification_results.iter().any(|r| *r == VerificationResult::Failed);

        request.status = if all_verified {
            request.completed_at = Some(chrono::Utc::now().timestamp_millis() as u64);
            request.verification = Some(ErasureVerification {
                id: Uuid::new_v4(),
                erasure_request_id: *request_id,
                method: VerificationMethod::DatabaseQuery,
                verified_at: chrono::Utc::now().timestamp_millis() as u64,
                result: VerificationResult::Verified,
                details: HashMap::new(),
            });
            ErasureStatus::Completed
        } else if any_failed {
            ErasureStatus::Failed
        } else {
            ErasureStatus::Completed
        };

        info!(
            "Erasure request {} completed with status {:?}",
            request_id, request.status
        );
        Ok(request.clone())
    }

    /// 获取删除请求状态
    pub async fn get_request(&self, request_id: &Uuid) -> Option<ErasureRequest> {
        let requests = self.requests.read().await;
        requests.get(request_id).cloned()
    }

    /// 获取用户所有删除请求
    pub async fn get_user_requests(&self, user_id: &str) -> Vec<ErasureRequest> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.user_id == user_id)
            .cloned()
            .collect()
    }

    /// 拒绝删除请求 (存在法律义务)
    pub async fn reject_request(
        &self,
        request_id: &Uuid,
        reason: &str,
    ) -> Result<ErasureRequest> {
        let mut requests = self.requests.write().await;

        let request = requests
            .get_mut(request_id)
            .ok_or_else(|| super::GdprError::UserNotFound(request_id.to_string()))?;

        request.status = ErasureStatus::Rejected;
        request.notes = Some(reason.to_string());
        request.completed_at = Some(chrono::Utc::now().timestamp_millis() as u64);

        warn!(
            "Erasure request {} rejected: {}",
            request_id, reason
        );
        Ok(request.clone())
    }
}

impl Default for DataErasureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erasure_request_creation() {
        let request = ErasureRequest::new(
            "user-1",
            "tenant-1",
            ErasureType::FullErasure,
            vec!["profile".to_string()],
        );

        assert_eq!(request.status, ErasureStatus::Pending);
        assert!(request.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_erasure_manager() {
        let manager = DataErasureManager::new();

        let request = manager
            .create_request(
                "user-1",
                "tenant-1",
                ErasureType::PartialErasure,
                vec!["messages".to_string()],
            )
            .await
            .unwrap();

        assert!(manager.get_request(&request.id).await.is_some());
    }
}
