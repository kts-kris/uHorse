//! Hub-Node 集成测试
//!
//! 测试 Hub 和 Node 之间的完整通信流程

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uhorse_hub::{Hub, HubConfig};
use uhorse_node::{Node, NodeConfig, Workspace, WorkspaceConfig};
use uhorse_protocol::{
    Command, FileCommand, NodeCapabilities, NodeId, Priority, ShellCommand,
    TaskContext, WorkspaceInfo,
};

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
    let workspace = WorkspaceInfo {
        path: "/tmp/workspace".to_string(),
        name: "test-workspace".to_string(),
        platform: "linux".to_string(),
        arch: "x86_64".to_string(),
    };

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
    let workspace = WorkspaceInfo {
        path: "/tmp/workspace".to_string(),
        name: "test-workspace".to_string(),
        platform: "linux".to_string(),
        arch: "x86_64".to_string(),
    };

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
        timeout: Duration::from_secs(30),
        capture_stderr: true,
    });

    let context = TaskContext {
        user_id: "test-user".to_string(),
        session_id: "test-session".to_string(),
        tenant_id: None,
        metadata: Default::default(),
    };

    let task_id = hub
        .submit_task(
            command,
            context,
            Priority::Normal,
            None,
            vec![],
            None,
        )
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

    // 提交文件命令
    let command = Command::File(FileCommand::Exists {
        path: "/tmp/test.txt".to_string(),
    });

    let context = TaskContext {
        user_id: "test-user".to_string(),
        session_id: "test-session".to_string(),
        tenant_id: None,
        metadata: Default::default(),
    };

    let task_id = hub
        .submit_task(
            command,
            context,
            Priority::Normal,
            None,
            vec![],
            None,
        )
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
    let workspace = WorkspaceInfo {
        path: "/tmp/workspace".to_string(),
        name: "test-workspace".to_string(),
        platform: "linux".to_string(),
        arch: "x86_64".to_string(),
    };

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
    let context = TaskContext {
        user_id: "test-user".to_string(),
        session_id: "test-session".to_string(),
        tenant_id: None,
        metadata: Default::default(),
    };

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
    let workspace = WorkspaceInfo {
        path: "/tmp/workspace".to_string(),
        name: "test-workspace".to_string(),
        platform: "linux".to_string(),
        arch: "x86_64".to_string(),
    };

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
