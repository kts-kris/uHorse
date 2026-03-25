//! Hub-Node 集成测试
//!
//! 测试 Hub 和 Node 之间的完整通信流程

use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use tempfile::tempdir;
use tokio::time::timeout;
use uhorse_hub::{create_router, Hub, HubConfig, NodeAuthenticator, WebState};
use uhorse_node_runtime::{ConnectionConfig, Node, NodeConfig};
use uhorse_protocol::{
    Command, CommandOutput, FileCommand, NodeCapabilities, NodeId, Priority, SessionId,
    ShellCommand, TaskContext, UserId, WorkspaceInfo,
};

/// 创建测试用的工作空间信息
fn create_test_workspace(name: &str, path: &str) -> WorkspaceInfo {
    WorkspaceInfo {
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
        "test-channel",
    )
}

/// 测试 Hub 创建
#[tokio::test]
async fn test_hub_creation() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        bind_address: "127.0.0.1".to_string(),
        port: 18080,
        max_nodes: 10,
        heartbeat_timeout_secs: 30,
        task_timeout_secs: 60,
        max_retries: 3,
    };

    let (hub, _rx) = Hub::new(config);
    assert_eq!(hub.hub_id(), "test-hub");
}

/// 测试节点注册
#[tokio::test]
async fn test_node_registration() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node-1");
    let capabilities = NodeCapabilities::default();
    let workspace = create_test_workspace("test-workspace", "/tmp/workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Test Node".to_string(),
        capabilities,
        workspace,
        vec!["test".to_string()],
    )
    .await
    .unwrap();

    // 验证节点已注册
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_id, node_id);
}

/// 测试任务提交
#[tokio::test]
async fn test_task_submission() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node-1");
    let capabilities = NodeCapabilities {
        max_concurrent_tasks: 10,
        ..Default::default()
    };
    let workspace = create_test_workspace("test-workspace", "/tmp/workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Test Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 提交任务
    let command = Command::Shell(ShellCommand {
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: None,
        env: Default::default(),
        timeout: std::time::Duration::from_secs(30),
        capture_stderr: true,
    });

    let context = create_test_context("test-user", "test-session");

    let task_id = hub
        .submit_task(command, context, Priority::Normal, None, vec![], None)
        .await
        .unwrap();

    // 验证任务已创建
    assert!(!task_id.as_str().is_empty());
}

/// 测试文件命令
#[tokio::test]
async fn test_file_command() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node-1");
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

    // 提交文件命令
    let command = Command::File(FileCommand::Exists {
        path: "/tmp/test.txt".to_string(),
    });

    let context = create_test_context("test-user", "test-session");

    let task_id = hub
        .submit_task(command, context, Priority::Normal, None, vec![], None)
        .await
        .unwrap();

    assert!(!task_id.as_str().is_empty());
}

/// 测试优先级调度
#[tokio::test]
async fn test_priority_scheduling() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node-1");
    let capabilities = NodeCapabilities {
        max_concurrent_tasks: 10,
        ..Default::default()
    };
    let workspace = create_test_workspace("test-workspace", "/tmp/workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Test Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 提交不同优先级的任务
    let context = create_test_context("test-user", "test-session");

    let low_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 1")),
            context.clone(),
            Priority::Low,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let high_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 1")),
            context.clone(),
            Priority::High,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let critical_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 1")),
            context,
            Priority::Critical,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    // 验证所有任务都已创建
    assert!(!low_task.as_str().is_empty());
    assert!(!high_task.as_str().is_empty());
    assert!(!critical_task.as_str().is_empty());
}

/// 测试统计信息
#[tokio::test]
async fn test_hub_stats() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 获取统计信息
    let stats = hub.get_stats().await;
    assert_eq!(stats.hub_id, "test-hub");
    assert_eq!(stats.nodes.total_nodes, 0);
    assert_eq!(stats.scheduler.pending_tasks, 0);
}

/// 测试节点断开
#[tokio::test]
async fn test_node_disconnect() {
    let config = HubConfig {
        hub_id: "test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("test-node-1");
    let capabilities = NodeCapabilities::default();
    let workspace = create_test_workspace("test-workspace", "/tmp/workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Test Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 验证节点在线
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 1);

    // 断开节点
    hub.handle_node_disconnect(&node_id).await.unwrap();

    // 验证节点已断开
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 0);
}

#[tokio::test]
async fn test_local_hub_rejects_node_with_mismatched_auth_token() {
    let workspace = tempdir().unwrap();

    let hub_config = HubConfig {
        hub_id: "roundtrip-auth-reject-hub".to_string(),
        bind_address: "127.0.0.1".to_string(),
        port: 18764,
        heartbeat_timeout_secs: 10,
        task_timeout_secs: 30,
        ..Default::default()
    };

    let jwt_secret = "roundtrip-test-secret-12345";
    let authenticator = NodeAuthenticator::with_secret(jwt_secret).unwrap();
    let token_node_id = NodeId::from_string("token-node");
    let auth_info = authenticator
        .authenticate_node(&token_node_id, "roundtrip-credentials")
        .await
        .unwrap();

    let security_manager = Arc::new(
        uhorse_hub::SecurityManager::new(
            jwt_secret,
            Arc::new(uhorse_security::ApprovalManager::new()),
        )
        .unwrap(),
    );
    let (hub, _task_result_rx) = Hub::new_with_security(hub_config.clone(), Some(security_manager));
    let hub = Arc::new(hub);
    hub.start().await.unwrap();

    let web_state = WebState::new(hub.clone(), None, None);
    let app = create_router(web_state);
    let listener =
        tokio::net::TcpListener::bind((hub_config.bind_address.as_str(), hub_config.port))
            .await
            .unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut node = Node::new(NodeConfig {
        node_id: Some(NodeId::from_string("registered-node")),
        name: "roundtrip-node".to_string(),
        workspace_path: workspace.path().to_string_lossy().to_string(),
        connection: ConnectionConfig {
            hub_url: format!("ws://127.0.0.1:{}/ws", hub_config.port),
            reconnect_interval_secs: 1,
            heartbeat_interval_secs: 1,
            connect_timeout_secs: 5,
            max_reconnect_attempts: 1,
            auth_token: Some(auth_info.access_token.clone()),
        },
        require_git_repo: false,
        ..Default::default()
    })
    .unwrap();

    node.start().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert!(hub.get_online_nodes().await.is_empty());

    node.stop().await.unwrap();
    server.abort();
    let _ = server.await;
    hub.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_metrics_follow_websocket_and_http_paths() {
    let workspace = tempdir().unwrap();

    let hub_config = HubConfig {
        hub_id: "metrics-path-hub".to_string(),
        bind_address: "127.0.0.1".to_string(),
        port: 18766,
        heartbeat_timeout_secs: 10,
        task_timeout_secs: 30,
        ..Default::default()
    };

    let jwt_secret = "metrics-test-secret-12345";
    let authenticator = NodeAuthenticator::with_secret(jwt_secret).unwrap();
    let node_id = NodeId::from_string("metrics-authenticated-node");
    let auth_info = authenticator
        .authenticate_node(&node_id, "metrics-credentials")
        .await
        .unwrap();

    let security_manager = Arc::new(
        uhorse_hub::SecurityManager::new(
            jwt_secret,
            Arc::new(uhorse_security::ApprovalManager::new()),
        )
        .unwrap(),
    );
    let (hub, _task_result_rx) = Hub::new_with_security(hub_config.clone(), Some(security_manager));
    let hub = Arc::new(hub);
    hub.start().await.unwrap();

    let web_state = WebState::new(hub.clone(), None, None);
    let app = create_router(web_state);
    let listener =
        tokio::net::TcpListener::bind((hub_config.bind_address.as_str(), hub_config.port))
            .await
            .unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let http_client = reqwest::Client::new();
    let metrics_url = format!("http://127.0.0.1:{}/metrics", hub_config.port);
    let health_url = format!("http://127.0.0.1:{}/api/health", hub_config.port);

    let mut node = Node::new(NodeConfig {
        node_id: Some(node_id.clone()),
        name: "metrics-node".to_string(),
        workspace_path: workspace.path().to_string_lossy().to_string(),
        connection: ConnectionConfig {
            hub_url: format!("ws://127.0.0.1:{}/ws", hub_config.port),
            reconnect_interval_secs: 1,
            heartbeat_interval_secs: 1,
            connect_timeout_secs: 5,
            max_reconnect_attempts: 1,
            auth_token: Some(auth_info.access_token.clone()),
        },
        require_git_repo: false,
        ..Default::default()
    })
    .unwrap();

    let baseline_metrics = http_client
        .get(&metrics_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(baseline_metrics.contains("uhorse_api_requests_total 0"));
    assert!(baseline_metrics.contains("uhorse_websocket_connections 0"));

    node.start().await.unwrap();

    timeout(Duration::from_secs(5), async {
        loop {
            if hub.get_online_nodes().await.len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    let health_response = http_client.get(&health_url).send().await.unwrap();
    assert_eq!(health_response.status(), StatusCode::OK);

    let metrics_after_health = http_client
        .get(&metrics_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics_after_health.contains("uhorse_api_requests_total 3"));
    assert!(metrics_after_health.contains("uhorse_websocket_connections 1"));

    node.stop().await.unwrap();

    timeout(Duration::from_secs(5), async {
        loop {
            if hub.get_online_nodes().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    let metrics_after_stop = http_client
        .get(&metrics_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics_after_stop.contains("uhorse_websocket_connections 0"));

    server.abort();
    let _ = server.await;
    hub.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_local_hub_node_roundtrip_file_exists() {
    let workspace = tempdir().unwrap();
    let existing_file = workspace.path().join("roundtrip.txt");
    std::fs::write(&existing_file, "ok").unwrap();

    let hub_config = HubConfig {
        hub_id: "roundtrip-test-hub".to_string(),
        bind_address: "127.0.0.1".to_string(),
        port: 18765,
        heartbeat_timeout_secs: 10,
        task_timeout_secs: 30,
        ..Default::default()
    };

    let jwt_secret = "roundtrip-test-secret-12345";
    let authenticator = NodeAuthenticator::with_secret(jwt_secret).unwrap();
    let node_id = NodeId::from_string("roundtrip-authenticated-node");
    let auth_info = authenticator
        .authenticate_node(&node_id, "roundtrip-credentials")
        .await
        .unwrap();

    let security_manager = Arc::new(
        uhorse_hub::SecurityManager::new(
            jwt_secret,
            Arc::new(uhorse_security::ApprovalManager::new()),
        )
        .unwrap(),
    );
    let (hub, mut task_result_rx) =
        Hub::new_with_security(hub_config.clone(), Some(security_manager));
    let hub = Arc::new(hub);
    hub.start().await.unwrap();

    let web_state = WebState::new(hub.clone(), None, None);
    let app = create_router(web_state);
    let listener =
        tokio::net::TcpListener::bind((hub_config.bind_address.as_str(), hub_config.port))
            .await
            .unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut node = Node::new(NodeConfig {
        node_id: Some(node_id.clone()),
        name: "roundtrip-node".to_string(),
        workspace_path: workspace.path().to_string_lossy().to_string(),
        connection: ConnectionConfig {
            hub_url: format!("ws://127.0.0.1:{}/ws", hub_config.port),
            reconnect_interval_secs: 1,
            heartbeat_interval_secs: 1,
            connect_timeout_secs: 5,
            max_reconnect_attempts: 1,
            auth_token: Some(auth_info.access_token.clone()),
        },
        require_git_repo: false,
        ..Default::default()
    })
    .unwrap();

    node.start().await.unwrap();

    timeout(Duration::from_secs(5), async {
        loop {
            if hub.get_online_nodes().await.len() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    let task_id = hub
        .submit_task(
            Command::File(FileCommand::Exists {
                path: existing_file.to_string_lossy().to_string(),
            }),
            create_test_context("test-user", "roundtrip-session"),
            Priority::Normal,
            None,
            vec![],
            Some(workspace.path().to_string_lossy().to_string()),
        )
        .await
        .unwrap();

    let result = timeout(Duration::from_secs(10), task_result_rx.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(result.task_id, task_id);
    assert!(result.result.success);
    match &result.result.output {
        CommandOutput::Json { content } => {
            assert_eq!(
                content.get("exists").and_then(|value| value.as_bool()),
                Some(true)
            );
        }
        other => panic!("unexpected output: {:?}", other),
    }

    timeout(Duration::from_secs(5), async {
        loop {
            match hub.get_task_status(&task_id).await {
                Some(status) if format!("{:?}", status.status) == "Completed" => break,
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await
    .unwrap();

    node.stop().await.unwrap();
    server.abort();
    let _ = server.await;
    hub.shutdown().await.unwrap();
}
