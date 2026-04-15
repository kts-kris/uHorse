//! 安全测试
//!
//! 测试 Hub 的安全功能：JWT 认证、敏感操作审批、字段加密

use std::sync::Arc;
use uhorse_channel::ChannelRegistry;
use uhorse_hub::{
    security_integration::{
        HubFieldEncryptor, HubTlsConfig, NodeAuthenticator, SecurityManager,
        SensitiveOperationApprover,
    },
    Hub, HubConfig, MessageRouter, NodeManager, NotificationBindingManager, TaskScheduler,
};
use uhorse_protocol::{
    Command, MessageId, NodeCapabilities, NodeId, NodeToHub, Priority, SessionId, ShellCommand,
    TaskContext, TaskId, UserId, WorkspaceInfo,
};
use uhorse_security::{ApprovalLevel, ApprovalManager, ApprovalStatus, EncryptionKey};

/// 创建测试用的工作空间信息
fn create_test_workspace(name: &str, path: &str) -> WorkspaceInfo {
    WorkspaceInfo {
        workspace_id: None,
        name: name.to_string(),
        path: path.to_string(),
        read_only: false,
        allowed_patterns: vec!["*".to_string()],
        denied_patterns: vec![],
    }
}

/// 创建测试用的任务上下文
fn create_test_context(user: &str, session: &str) -> TaskContext {
    TaskContext::new(
        UserId::from_string(user),
        SessionId::from_string(session),
        "security-test-channel",
    )
}

fn create_test_message_router() -> MessageRouter {
    let node_manager = Arc::new(NodeManager::new(10, 30));
    let (task_scheduler, _rx) = TaskScheduler::new(node_manager.clone(), 3, 60);
    MessageRouter::new(
        node_manager,
        Arc::new(task_scheduler),
        Arc::new(ChannelRegistry::new()),
        Arc::new(NotificationBindingManager::default()),
    )
}

// ============================================================================
// JWT 认证测试
// ============================================================================

/// 测试节点认证器创建
#[tokio::test]
async fn test_node_authenticator_creation() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();

    // 验证认证器可以创建
    assert!(authenticator.verify_token("invalid-token").await.is_err());
}

/// 测试节点认证流程
#[tokio::test]
async fn test_node_authentication() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
    let node_id = NodeId::from_string("test-node");

    // 认证节点
    let auth_info = authenticator
        .authenticate_node(&node_id, "test-credentials")
        .await
        .unwrap();

    assert_eq!(auth_info.node_id, node_id);
    assert!(!auth_info.access_token.is_empty());
    assert!(!auth_info.refresh_token.is_empty());
    assert!(authenticator.is_authenticated(&node_id).await);
}

/// 测试令牌验证
#[tokio::test]
async fn test_token_verification() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
    let node_id = NodeId::from_string("test-node");

    // 认证并获取令牌
    let auth_info = authenticator
        .authenticate_node(&node_id, "test-credentials")
        .await
        .unwrap();

    // 验证令牌
    let verified_node_id = authenticator
        .verify_token(&auth_info.access_token)
        .await
        .unwrap();

    // 验证返回的节点 ID 不为空
    assert!(!verified_node_id.as_str().is_empty());
}

/// 测试无效令牌
#[tokio::test]
async fn test_invalid_token() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();

    // 验证无效令牌
    let result = authenticator.verify_token("invalid-token").await;
    assert!(result.is_err());
}

/// 测试令牌刷新
#[tokio::test]
async fn test_token_refresh() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
    let node_id = NodeId::from_string("test-node");

    // 认证并获取令牌
    let auth_info = authenticator
        .authenticate_node(&node_id, "test-credentials")
        .await
        .unwrap();

    // 刷新令牌
    let new_tokens = authenticator
        .refresh_token(&auth_info.refresh_token)
        .await
        .unwrap();

    assert!(!new_tokens.access_token.is_empty());
    assert!(!new_tokens.refresh_token.is_empty());
}

/// 测试节点注销
#[tokio::test]
async fn test_node_revocation() {
    let authenticator = NodeAuthenticator::with_secret("test-secret-key-12345").unwrap();
    let node_id = NodeId::from_string("test-node");

    // 认证节点
    authenticator
        .authenticate_node(&node_id, "test-credentials")
        .await
        .unwrap();

    assert!(authenticator.is_authenticated(&node_id).await);

    // 注销节点
    authenticator.revoke_node(&node_id).await.unwrap();

    assert!(!authenticator.is_authenticated(&node_id).await);
}

// ============================================================================
// 敏感操作审批测试
// ============================================================================

/// 测试审批器创建
#[tokio::test]
async fn test_approver_creation() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);

    // 验证审批器可以创建
    assert!(!approver.requires_approval("normal_operation"));
    assert!(approver.requires_approval("file_delete"));
}

/// 测试需要审批的操作检测
#[tokio::test]
async fn test_approval_requirement_detection() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);

    // 需要审批的操作
    assert!(approver.requires_approval("file_delete"));
    assert!(approver.requires_approval("system_command"));
    assert!(approver.requires_approval("network_access"));
    assert!(approver.requires_approval("credential_access"));
    assert!(approver.requires_approval("config_change"));

    // 不需要审批的操作
    assert!(!approver.requires_approval("file_read"));
    assert!(!approver.requires_approval("file_write"));
    assert!(!approver.requires_approval("normal_operation"));
}

/// 测试审批请求创建
#[tokio::test]
async fn test_approval_request_creation() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);
    let node_id = NodeId::from_string("test-node");

    // 创建审批请求
    let context = serde_json::json!({
        "operation": "delete_file",
        "path": "/tmp/test.txt"
    });

    let request_id = approver
        .request_approval(&node_id, "file_delete", ApprovalLevel::Single, context)
        .await
        .unwrap();

    assert!(!request_id.is_empty());
}

/// 测试审批通过
#[tokio::test]
async fn test_approval_approve() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);
    let node_id = NodeId::from_string("test-node");

    // 创建审批请求
    let request_id = approver
        .request_approval(
            &node_id,
            "system_command",
            ApprovalLevel::Single,
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // 获取状态（应该是待审批）
    let status = approver.get_approval_status(&request_id).await.unwrap();
    assert_eq!(status, uhorse_security::ApprovalStatus::Pending);

    // 批准
    approver
        .approve(&request_id, "admin", Some("Approved for maintenance"))
        .await
        .unwrap();

    // 验证状态已更新
    let status = approver.get_approval_status(&request_id).await.unwrap();
    assert_eq!(status, uhorse_security::ApprovalStatus::Approved);
}

/// 测试审批拒绝
#[tokio::test]
async fn test_approval_reject() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);
    let node_id = NodeId::from_string("test-node");

    // 创建审批请求
    let request_id = approver
        .request_approval(
            &node_id,
            "credential_access",
            ApprovalLevel::Single,
            serde_json::json!({}),
        )
        .await
        .unwrap();

    // 拒绝
    approver
        .reject(&request_id, "admin", Some("Risk too high"))
        .await
        .unwrap();

    // 验证状态已更新
    let status = approver.get_approval_status(&request_id).await.unwrap();
    assert_eq!(status, uhorse_security::ApprovalStatus::Rejected);
}

#[tokio::test]
async fn test_message_router_creates_hub_approval_from_node_request() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let security_manager = SecurityManager::new("jwt-secret", approval_manager.clone()).unwrap();
    let router = create_test_message_router();
    let node_id = NodeId::from_string("test-node");
    let task_id = TaskId::from_string("task-123");
    let request_id = "approval-req-123".to_string();

    router
        .route_node_message(
            &node_id,
            NodeToHub::ApprovalRequest {
                message_id: MessageId::new(),
                request_id: request_id.clone(),
                task_id: task_id.clone(),
                command: Command::Shell(ShellCommand::new("rm -rf /tmp/test")),
                context: create_test_context("test-user", "test-session"),
                reason: "dangerous command".to_string(),
                timestamp: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
            },
            Some(&security_manager),
            None,
            None,
        )
        .await
        .unwrap();

    let pending = approval_manager.list_pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].status, ApprovalStatus::Pending);
    assert_eq!(pending[0].action, "system_command");
    assert_eq!(pending[0].requested_by, node_id.as_str());
    assert_eq!(
        pending[0].metadata.get("request_id"),
        Some(&serde_json::json!(request_id))
    );
    assert_eq!(
        pending[0].metadata.get("task_id"),
        Some(&serde_json::json!(task_id.as_str()))
    );
}

#[tokio::test]
async fn test_message_router_rejects_approval_request_without_security_manager() {
    let router = create_test_message_router();
    let node_id = NodeId::from_string("test-node");

    let result = router
        .route_node_message(
            &node_id,
            NodeToHub::ApprovalRequest {
                message_id: MessageId::new(),
                request_id: "approval-req-456".to_string(),
                task_id: TaskId::from_string("task-456"),
                command: Command::Shell(ShellCommand::new("rm -rf /tmp/test")),
                context: create_test_context("test-user", "test-session"),
                reason: "dangerous command".to_string(),
                timestamp: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
            },
            None,
            None,
            None,
        )
        .await;

    assert!(matches!(result, Err(uhorse_hub::HubError::Permission(_))));
}

/// 测试幂等性检查
#[tokio::test]
async fn test_idempotency_check() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let approver = SensitiveOperationApprover::new(approval_manager);

    let operation_id = "op-123";

    // 第一次检查
    let is_duplicate = approver.check_idempotency(operation_id, 60).await.unwrap();
    assert!(!is_duplicate);

    // 存储响应
    approver
        .store_idempotency_response(operation_id, &serde_json::json!({"result": "ok"}), 60)
        .await
        .unwrap();

    // 第二次检查（应该是重复的）
    let is_duplicate = approver.check_idempotency(operation_id, 60).await.unwrap();
    assert!(is_duplicate);
}

// ============================================================================
// 字段加密测试
// ============================================================================

/// 测试加密器创建
#[tokio::test]
async fn test_encryptor_creation() {
    let key = EncryptionKey::new([0u8; 32]);
    let encryptor = HubFieldEncryptor::new(key);

    // 验证加密器可以创建
    let data = b"test-data";
    let encrypted = encryptor.encrypt(data).unwrap();
    assert!(!encrypted.ciphertext.is_empty());
}

/// 测试加密解密
#[tokio::test]
async fn test_encrypt_decrypt() {
    let key = EncryptionKey::new([1u8; 32]);
    let encryptor = HubFieldEncryptor::new(key);

    let original = b"sensitive-data-123";

    // 加密
    let encrypted = encryptor.encrypt(original).unwrap();
    assert_ne!(encrypted.ciphertext.as_slice(), original);

    // 解密
    let decrypted = encryptor.decrypt(&encrypted).unwrap();
    assert_eq!(decrypted.as_slice(), original);
}

/// 测试 JSON 加密
#[tokio::test]
async fn test_json_encryption() {
    let key = EncryptionKey::new([2u8; 32]);
    let encryptor = HubFieldEncryptor::new(key);

    let data = serde_json::json!({
        "api_key": "secret-key-12345",
        "password": "my-password"
    });

    // 加密
    let encrypted = encryptor.encrypt_json(&data).unwrap();

    // 解密
    let decrypted: serde_json::Value = encryptor.decrypt_json(&encrypted).unwrap();
    assert_eq!(decrypted, data);
}

/// 测试不同密钥
#[tokio::test]
async fn test_different_keys() {
    let key1 = EncryptionKey::new([1u8; 32]);
    let key2 = EncryptionKey::new([2u8; 32]);

    let encryptor1 = HubFieldEncryptor::new(key1);
    let encryptor2 = HubFieldEncryptor::new(key2);

    let original = b"secret-data";

    let encrypted1 = encryptor1.encrypt(original).unwrap();

    // 使用不同密钥解密应该失败
    let result = encryptor2.decrypt(&encrypted1);
    assert!(result.is_err());
}

/// 测试主密钥创建
#[tokio::test]
async fn test_master_key_creation() {
    let master_key = [42u8; 32];
    let encryptor = HubFieldEncryptor::with_master_key(master_key);

    let data = b"test";
    let encrypted = encryptor.encrypt(data).unwrap();
    let decrypted = encryptor.decrypt(&encrypted).unwrap();

    assert_eq!(decrypted.as_slice(), data);
}

// ============================================================================
// TLS 配置测试
// ============================================================================

/// 测试 TLS 配置创建
#[tokio::test]
async fn test_tls_config_creation() {
    let tls_config = HubTlsConfig::new("/path/to/cert.pem", "/path/to/key.pem");

    // 验证 TLS 配置可以创建
    let cert_path = tls_config.inner().cert_path.to_string_lossy();
    let key_path = tls_config.inner().key_path.to_string_lossy();
    assert!(cert_path.contains("cert.pem"));
    assert!(key_path.contains("key.pem"));
}

// ============================================================================
// 安全管理器集成测试
// ============================================================================

/// 测试安全管理器创建
#[tokio::test]
async fn test_security_manager_creation() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let manager = SecurityManager::new("jwt-secret", approval_manager).unwrap();

    // 验证安全管理器可以创建
    assert!(manager.field_encryptor().is_none());
    assert!(manager.tls_config().is_none());
}

/// 测试安全管理器完整流程
#[tokio::test]
async fn test_security_manager_workflow() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let manager = SecurityManager::new("jwt-secret", approval_manager)
        .unwrap()
        .with_field_encryption([3u8; 32]);

    let node_id = NodeId::from_string("secure-node");

    // 认证节点
    let auth_info = manager
        .node_authenticator()
        .authenticate_node(&node_id, "credentials")
        .await
        .unwrap();

    assert_eq!(auth_info.node_id, node_id);

    // 验证令牌
    let verified = manager
        .node_authenticator()
        .verify_token(&auth_info.access_token)
        .await
        .unwrap();

    assert!(!verified.as_str().is_empty());

    // 使用字段加密
    let encryptor = manager.field_encryptor().unwrap();
    let data = b"sensitive";
    let encrypted = encryptor.encrypt(data).unwrap();
    let decrypted = encryptor.decrypt(&encrypted).unwrap();
    assert_eq!(decrypted.as_slice(), data);
}

/// 测试安全管理器带 TLS
#[tokio::test]
async fn test_security_manager_with_tls() {
    let approval_manager = Arc::new(ApprovalManager::new());
    let manager = SecurityManager::new("jwt-secret", approval_manager)
        .unwrap()
        .with_tls("/etc/ssl/cert.pem", "/etc/ssl/key.pem");

    assert!(manager.tls_config().is_some());
}

// ============================================================================
// Hub 安全集成测试
// ============================================================================

/// 测试 Hub 的安全配置
#[tokio::test]
async fn test_hub_security_config() {
    let config = HubConfig {
        hub_id: "secure-hub".to_string(),
        max_nodes: 10,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    let stats = hub.get_stats().await;
    assert_eq!(stats.hub_id, "secure-hub");
}

/// 测试节点注册和认证
#[tokio::test]
async fn test_node_registration_with_auth() {
    let config = HubConfig {
        hub_id: "secure-hub".to_string(),
        max_nodes: 10,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("authenticated-node");
    let workspace = create_test_workspace("secure-workspace", "/tmp/secure");

    hub.handle_node_connection(
        node_id.clone(),
        "Secure Node".to_string(),
        NodeCapabilities::default(),
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 验证节点在线
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_id, node_id);
}

/// 测试敏感命令提交
#[tokio::test]
async fn test_sensitive_command_submission() {
    let config = HubConfig {
        hub_id: "secure-hub".to_string(),
        max_nodes: 10,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node");
    let workspace = create_test_workspace("test-workspace", "/tmp/workspace");

    hub.handle_node_connection(
        node_id,
        "Test Node".to_string(),
        NodeCapabilities::default(),
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 提交敏感命令
    let context = create_test_context("test-user", "test-session");

    let result = hub
        .submit_task(
            Command::Shell(ShellCommand::new("rm -rf /tmp/test")),
            context,
            Priority::High,
            None,
            vec![],
            None,
        )
        .await;

    // 任务应该被提交
    assert!(result.is_ok());
}

// ============================================================================
// 访问控制测试
// ============================================================================

/// 测试工作空间访问控制
#[tokio::test]
async fn test_workspace_access_control() {
    // 只读工作空间
    let readonly_workspace = WorkspaceInfo {
        workspace_id: None,
        name: "readonly-workspace".to_string(),
        path: "/readonly".to_string(),
        read_only: true,
        allowed_patterns: vec!["*.txt".to_string()],
        denied_patterns: vec!["secret/*".to_string()],
    };

    // 验证只读标志
    assert!(readonly_workspace.read_only);
    assert!(readonly_workspace
        .allowed_patterns
        .contains(&"*.txt".to_string()));
    assert!(readonly_workspace
        .denied_patterns
        .contains(&"secret/*".to_string()));

    // 完全访问工作空间
    let full_workspace = WorkspaceInfo {
        workspace_id: None,
        name: "full-workspace".to_string(),
        path: "/full".to_string(),
        read_only: false,
        allowed_patterns: vec!["*".to_string()],
        denied_patterns: vec![],
    };

    assert!(!full_workspace.read_only);
}

/// 测试节点能力限制
#[tokio::test]
async fn test_node_capability_limits() {
    let limited_capabilities = NodeCapabilities {
        supported_commands: vec![uhorse_protocol::CommandType::File],
        tags: vec!["limited".to_string()],
        max_concurrent_tasks: 1,
        available_tools: vec!["cat".to_string()],
    };

    // 验证能力限制
    assert_eq!(limited_capabilities.max_concurrent_tasks, 1);
    assert_eq!(limited_capabilities.supported_commands.len(), 1);
    assert!(limited_capabilities.tags.contains(&"limited".to_string()));
}
