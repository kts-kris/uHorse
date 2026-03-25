//! 安全集成模块
//!
//! 复用 uhorse-security 的能力实现 Hub-Node 安全通信：
//! - 节点认证
//! - 通信加密
//! - 敏感操作保护

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use uhorse_protocol::NodeId;
use uhorse_security::{
    approval::Approver, ApprovalLevel, ApprovalManager, ApprovalStatus, EncryptedField,
    EncryptionKey, FieldEncryptor, IdempotencyCache, JwtAuthService, TlsConfig, TokenPair,
};

use crate::error::{HubError, HubResult};

/// 节点认证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAuthInfo {
    /// 节点 ID
    pub node_id: NodeId,
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// 认证时间
    pub authenticated_at: DateTime<Utc>,
}

/// 节点认证器
///
/// 复用 uhorse-security 的 JwtAuthService
#[derive(Debug)]
pub struct NodeAuthenticator {
    /// JWT 认证服务
    jwt_service: Arc<JwtAuthService>,
    /// 已认证节点缓存
    authenticated_nodes: Arc<tokio::sync::RwLock<std::collections::HashMap<NodeId, NodeAuthInfo>>>,
}

impl NodeAuthenticator {
    /// 创建新的节点认证器
    pub fn new(jwt_service: Arc<JwtAuthService>) -> Self {
        Self {
            jwt_service,
            authenticated_nodes: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// 使用密钥创建认证器
    pub fn with_secret(secret: &str) -> HubResult<Self> {
        let jwt_service = Arc::new(JwtAuthService::new(secret.to_string()));
        Ok(Self::new(jwt_service))
    }

    /// 认证节点
    pub async fn authenticate_node(
        &self,
        node_id: &NodeId,
        credentials: &str,
    ) -> HubResult<NodeAuthInfo> {
        if credentials.is_empty() {
            return Err(HubError::Permission("Empty credentials".to_string()));
        }

        // 生成令牌对
        let token_pair = self
            .jwt_service
            .create_token_pair(
                None,
                Some(node_id.as_str().to_string()),
                vec!["node".to_string()],
            )
            .await
            .map_err(|e| HubError::Permission(format!("Token generation failed: {}", e)))?;

        let auth_info = NodeAuthInfo {
            node_id: node_id.clone(),
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            authenticated_at: Utc::now(),
        };

        // 缓存认证信息
        {
            let mut nodes = self.authenticated_nodes.write().await;
            nodes.insert(node_id.clone(), auth_info.clone());
        }

        info!("Node {} authenticated successfully", node_id);
        Ok(auth_info)
    }

    /// 验证节点令牌
    pub async fn verify_token(&self, token: &str) -> HubResult<NodeId> {
        let access_token = self
            .jwt_service
            .verify_with_auto_refresh(token)
            .await
            .map_err(|e| HubError::Permission(format!("Token verification failed: {}", e)))?;

        // 使用 user_id 作为节点标识
        let node_id = access_token
            .user_id
            .unwrap_or_else(|| "unknown".to_string());
        Ok(NodeId::from_string(node_id))
    }

    /// 刷新令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> HubResult<TokenPair> {
        let token_pair = self
            .jwt_service
            .refresh_access_token(refresh_token)
            .await
            .map_err(|e| HubError::Permission(format!("Token refresh failed: {}", e)))?;

        Ok(token_pair)
    }

    /// 注销节点
    pub async fn revoke_node(&self, node_id: &NodeId) -> HubResult<()> {
        let mut nodes = self.authenticated_nodes.write().await;
        if nodes.remove(node_id).is_some() {
            info!("Node {} revoked", node_id);
        }
        Ok(())
    }

    /// 检查节点是否已认证
    pub async fn is_authenticated(&self, node_id: &NodeId) -> bool {
        let nodes = self.authenticated_nodes.read().await;
        if let Some(info) = nodes.get(node_id) {
            info.expires_at > Utc::now()
        } else {
            false
        }
    }
}

/// 敏感操作审批器
///
/// 复用 uhorse-security 的 ApprovalManager
pub struct SensitiveOperationApprover {
    /// 审批管理器
    approval_manager: Arc<ApprovalManager>,
    /// 幂等性缓存
    idempotency_cache: Arc<IdempotencyCache>,
}

impl SensitiveOperationApprover {
    /// 创建新的审批器
    pub fn new(approval_manager: Arc<ApprovalManager>) -> Self {
        let idempotency_cache = Arc::new(IdempotencyCache::new());
        Self {
            approval_manager,
            idempotency_cache,
        }
    }

    /// 请求审批
    pub async fn request_approval(
        &self,
        node_id: &NodeId,
        operation: &str,
        approval_level: ApprovalLevel,
        context: serde_json::Value,
    ) -> HubResult<String> {
        let approvers = vec![Approver {
            user_id: "hub-admin".to_string(),
            name: "Hub Administrator".to_string(),
            role: "admin".to_string(),
        }];

        let request = self
            .approval_manager
            .create_request(
                operation.to_string(),
                node_id.as_str().to_string(),
                approval_level,
                approvers,
                context,
            )
            .await
            .map_err(|e| HubError::Permission(format!("Failed to create approval: {}", e)))?;

        info!(
            "Approval request {} created for operation {}",
            request.id, operation
        );
        Ok(request.id)
    }

    /// 检查操作是否需要审批
    pub fn requires_approval(&self, operation: &str) -> bool {
        matches!(
            operation,
            "file_delete"
                | "system_command"
                | "network_access"
                | "credential_access"
                | "config_change"
        )
    }

    /// 获取审批状态
    pub async fn get_approval_status(&self, request_id: &str) -> HubResult<ApprovalStatus> {
        let request = self
            .approval_manager
            .get_request(request_id)
            .await
            .map_err(|e| HubError::Permission(format!("Failed to get approval: {}", e)))?
            .ok_or_else(|| {
                HubError::Permission(format!("Approval request {} not found", request_id))
            })?;

        Ok(request.status)
    }

    /// 获取审批请求
    pub async fn get_request(
        &self,
        request_id: &str,
    ) -> HubResult<Option<uhorse_security::ApprovalRequest>> {
        self.approval_manager
            .get_request(request_id)
            .await
            .map_err(|e| HubError::Permission(format!("Failed to get approval: {}", e)))
    }

    /// 列出待审批请求
    pub async fn list_pending_requests(&self) -> HubResult<Vec<uhorse_security::ApprovalRequest>> {
        self.approval_manager
            .list_pending()
            .await
            .map_err(|e| HubError::Permission(format!("Failed to list approvals: {}", e)))
    }

    /// 批准操作
    pub async fn approve(
        &self,
        request_id: &str,
        approver: &str,
        comment: Option<&str>,
    ) -> HubResult<()> {
        self.approval_manager
            .approve_request(
                request_id,
                approver.to_string(),
                comment.map(|s| s.to_string()),
            )
            .await
            .map_err(|e| HubError::Permission(format!("Failed to approve: {}", e)))?;

        info!("Approval {} approved by {}", request_id, approver);
        Ok(())
    }

    /// 拒绝操作
    pub async fn reject(
        &self,
        request_id: &str,
        rejecter: &str,
        comment: Option<&str>,
    ) -> HubResult<()> {
        self.approval_manager
            .reject_request(
                request_id,
                rejecter.to_string(),
                comment.map(|s| s.to_string()),
            )
            .await
            .map_err(|e| HubError::Permission(format!("Failed to reject: {}", e)))?;

        info!("Approval {} rejected by {}", request_id, rejecter);
        Ok(())
    }

    /// 检查幂等性
    pub async fn check_idempotency(&self, operation_id: &str, ttl_seconds: u64) -> HubResult<bool> {
        use uhorse_core::IdempotencyService;

        let result = self
            .idempotency_cache
            .check_or_record(operation_id, ttl_seconds)
            .await
            .map_err(|e| HubError::Internal(format!("Idempotency check failed: {}", e)))?;

        if result.is_some() {
            debug!("Operation {} already processed (idempotent)", operation_id);
            return Ok(true);
        }

        Ok(false)
    }

    /// 存储幂等性响应
    pub async fn store_idempotency_response(
        &self,
        operation_id: &str,
        response: &serde_json::Value,
        ttl_seconds: u64,
    ) -> HubResult<()> {
        use uhorse_core::IdempotencyService;

        self.idempotency_cache
            .store_response(operation_id, response, ttl_seconds)
            .await
            .map_err(|e| {
                HubError::Internal(format!("Failed to store idempotency response: {}", e))
            })?;

        Ok(())
    }
}

impl std::fmt::Debug for SensitiveOperationApprover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SensitiveOperationApprover").finish()
    }
}

/// TLS 配置包装
#[derive(Debug, Clone)]
pub struct HubTlsConfig {
    /// 内部 TLS 配置
    inner: TlsConfig,
}

impl HubTlsConfig {
    /// 创建新的 TLS 配置
    pub fn new(cert_path: &str, key_path: &str) -> Self {
        Self {
            inner: TlsConfig::new(cert_path, key_path),
        }
    }

    /// 获取内部配置
    pub fn inner(&self) -> &TlsConfig {
        &self.inner
    }
}

/// 字段加密器
///
/// 复用 uhorse-security 的字段加密能力
pub struct HubFieldEncryptor {
    /// 加密器
    encryptor: Arc<FieldEncryptor>,
}

impl HubFieldEncryptor {
    /// 创建新的加密器
    pub fn new(key: EncryptionKey) -> Self {
        Self {
            encryptor: Arc::new(FieldEncryptor::new(key)),
        }
    }

    /// 使用主密钥创建
    pub fn with_master_key(master_key: [u8; 32]) -> Self {
        let key = EncryptionKey::new(master_key);
        Self::new(key)
    }

    /// 加密数据
    pub fn encrypt(&self, data: &[u8]) -> HubResult<EncryptedField> {
        self.encryptor
            .encrypt(data)
            .map_err(|e| HubError::Internal(format!("Encryption failed: {}", e)))
    }

    /// 解密数据
    pub fn decrypt(&self, encrypted: &EncryptedField) -> HubResult<Vec<u8>> {
        self.encryptor
            .decrypt(encrypted)
            .map_err(|e| HubError::Internal(format!("Decryption failed: {}", e)))
    }

    /// 加密 JSON 数据
    pub fn encrypt_json<T: Serialize>(&self, data: &T) -> HubResult<EncryptedField> {
        self.encryptor
            .encrypt_json(data)
            .map_err(|e| HubError::Internal(format!("JSON encryption failed: {}", e)))
    }

    /// 解密 JSON 数据
    pub fn decrypt_json<T: for<'de> Deserialize<'de>>(
        &self,
        encrypted: &EncryptedField,
    ) -> HubResult<T> {
        self.encryptor
            .decrypt_json(encrypted)
            .map_err(|e| HubError::Internal(format!("JSON decryption failed: {}", e)))
    }
}

impl std::fmt::Debug for HubFieldEncryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubFieldEncryptor").finish()
    }
}

/// 安全管理器
///
/// 整合所有安全组件
pub struct SecurityManager {
    /// 节点认证器
    node_authenticator: NodeAuthenticator,
    /// 敏感操作审批器
    operation_approver: SensitiveOperationApprover,
    /// 字段加密器（可选）
    field_encryptor: Option<HubFieldEncryptor>,
    /// TLS 配置（可选）
    tls_config: Option<HubTlsConfig>,
}

impl SecurityManager {
    /// 创建新的安全管理器
    pub fn new(jwt_secret: &str, approval_manager: Arc<ApprovalManager>) -> HubResult<Self> {
        let node_authenticator = NodeAuthenticator::with_secret(jwt_secret)?;
        let operation_approver = SensitiveOperationApprover::new(approval_manager);

        Ok(Self {
            node_authenticator,
            operation_approver,
            field_encryptor: None,
            tls_config: None,
        })
    }

    /// 启用 TLS
    pub fn with_tls(mut self, cert_path: &str, key_path: &str) -> Self {
        self.tls_config = Some(HubTlsConfig::new(cert_path, key_path));
        self
    }

    /// 启用字段加密
    pub fn with_field_encryption(mut self, master_key: [u8; 32]) -> Self {
        self.field_encryptor = Some(HubFieldEncryptor::with_master_key(master_key));
        self
    }

    /// 获取节点认证器
    pub fn node_authenticator(&self) -> &NodeAuthenticator {
        &self.node_authenticator
    }

    /// 获取操作审批器
    pub fn operation_approver(&self) -> &SensitiveOperationApprover {
        &self.operation_approver
    }

    /// 获取字段加密器
    pub fn field_encryptor(&self) -> Option<&HubFieldEncryptor> {
        self.field_encryptor.as_ref()
    }

    /// 获取 TLS 配置
    pub fn tls_config(&self) -> Option<&HubTlsConfig> {
        self.tls_config.as_ref()
    }
}

impl std::fmt::Debug for SecurityManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityManager")
            .field("node_authenticator", &self.node_authenticator)
            .field("field_encryptor", &self.field_encryptor.is_some())
            .field("tls_config", &self.tls_config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_authenticator() {
        let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
        let node_id = NodeId::from_string("test-node");

        let auth_info = authenticator
            .authenticate_node(&node_id, "test-credentials")
            .await
            .unwrap();

        assert_eq!(auth_info.node_id, node_id);
        assert!(authenticator.is_authenticated(&node_id).await);
    }

    #[tokio::test]
    async fn test_token_verification() {
        let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
        let node_id = NodeId::from_string("test-node");

        let auth_info = authenticator
            .authenticate_node(&node_id, "test-credentials")
            .await
            .unwrap();

        let verified = authenticator
            .verify_token(&auth_info.access_token)
            .await
            .unwrap();

        // 验证返回的节点 ID（可能是从 user_id 提取的）
        assert!(!verified.as_str().is_empty());
    }
}
