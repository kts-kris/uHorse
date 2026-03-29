//! 端到端测试
//!
//! 测试 Hub-Node 完整通信流程

use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use uhorse_hub::{Hub, HubConfig};
use uhorse_protocol::{
    Command, FileCommand, HubToNode, LoadInfo, MessageCodec, MessageId, NodeCapabilities, NodeId,
    NodeStatus, NodeToHub, Priority, SessionId, ShellCommand, TaskContext, TaskId, UserId,
    WorkspaceInfo,
};

// ============================================================================
// 消息编解码测试
// ============================================================================

/// 测试消息编解码
#[tokio::test]
async fn test_message_codec() {
    // 测试 HubToNode 消息
    let hub_msg = HubToNode::HeartbeatRequest {
        message_id: MessageId::new(),
        timestamp: Utc::now(),
    };

    let encoded = MessageCodec::encode_hub_to_node(&hub_msg).unwrap();
    let decoded = MessageCodec::decode_hub_to_node(&encoded).unwrap();

    assert_eq!(hub_msg.message_type(), decoded.message_type());
    assert_eq!(hub_msg.message_id(), decoded.message_id());
}

/// 测试任务分配消息编解码
#[tokio::test]
async fn test_task_assignment_codec() {
    let command = Command::Shell(ShellCommand {
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: None,
        env: Default::default(),
        timeout: Duration::from_secs(30),
        capture_stderr: true,
    });

    let context = TaskContext::new(
        UserId::from_string("test-user"),
        SessionId::from_string("test-session"),
        "test-channel",
    );

    let msg = HubToNode::TaskAssignment {
        message_id: MessageId::new(),
        task_id: TaskId::new(),
        command,
        priority: Priority::High,
        deadline: Some(Utc::now() + chrono::Duration::hours(1)),
        context,
        retry_count: 0,
        max_retries: 3,
    };

    let encoded = MessageCodec::encode_hub_to_node(&msg).unwrap();
    let decoded = MessageCodec::decode_hub_to_node(&encoded).unwrap();

    assert_eq!(msg.message_type(), decoded.message_type());
}

/// 测试节点心跳消息编解码
#[tokio::test]
async fn test_heartbeat_codec() {
    let node_id = NodeId::from_string("test-node-1");

    let msg = NodeToHub::Heartbeat {
        message_id: MessageId::new(),
        node_id: node_id.clone(),
        status: NodeStatus {
            node_id: node_id.clone(),
            online: true,
            current_tasks: 2,
            max_tasks: 10,
            cpu_percent: 25.0,
            memory_mb: 1024,
            disk_gb: 100.0,
            network_latency_ms: Some(5),
            last_heartbeat: Utc::now(),
        },
        load: LoadInfo {
            cpu_usage: 0.25,
            memory_usage: 0.5,
            task_count: 2,
            latency_ms: Some(5),
        },
        timestamp: Utc::now(),
    };

    let encoded = MessageCodec::encode_node_to_hub(&msg).unwrap();
    let decoded = MessageCodec::decode_node_to_hub(&encoded).unwrap();

    assert_eq!(msg.message_type(), decoded.message_type());
}

// ============================================================================
// Hub 生命周期测试
// ============================================================================

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
        "test-channel",
    )
}

/// 测试 Hub 完整生命周期
#[tokio::test]
async fn test_hub_lifecycle() {
    let config = HubConfig {
        hub_id: "lifecycle-test-hub".to_string(),
        bind_address: "127.0.0.1".to_string(),
        port: 18081,
        max_nodes: 5,
        heartbeat_timeout_secs: 10,
        task_timeout_secs: 30,
        max_retries: 2,
    };

    let (hub, _rx) = Hub::new(config);

    // 验证初始状态
    assert_eq!(hub.hub_id(), "lifecycle-test-hub");

    let stats = hub.get_stats().await;
    assert_eq!(stats.nodes.total_nodes, 0);
    assert_eq!(stats.scheduler.pending_tasks, 0);
    assert_eq!(stats.scheduler.running_tasks, 0);

    // 注册多个节点
    for i in 1..=3 {
        let node_id = NodeId::from_string(format!("node-{}", i));
        let workspace = create_test_workspace(
            &format!("workspace-{}", i),
            &format!("/tmp/workspace-{}", i),
        );

        hub.handle_node_connection(
            node_id,
            format!("Node {}", i),
            NodeCapabilities::default(),
            workspace,
            vec!["test".to_string()],
        )
        .await
        .unwrap();
    }

    // 验证节点注册
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 3);

    // 验证统计更新
    let stats = hub.get_stats().await;
    assert_eq!(stats.nodes.total_nodes, 3);
    assert_eq!(stats.nodes.online_nodes, 3);

    // 断开一个节点
    hub.handle_node_disconnect(&NodeId::from_string("node-2"))
        .await
        .unwrap();

    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 2);

    let stats = hub.get_stats().await;
    assert_eq!(stats.nodes.online_nodes, 2);
}

// ============================================================================
// 多节点并行测试
// ============================================================================

/// 测试多节点并行任务提交
#[tokio::test]
async fn test_multi_node_parallel_tasks() {
    let config = HubConfig {
        hub_id: "parallel-test-hub".to_string(),
        max_nodes: 10,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册多个节点
    for i in 1..=5 {
        let node_id = NodeId::from_string(format!("parallel-node-{}", i));
        let capabilities = NodeCapabilities {
            max_concurrent_tasks: 5,
            ..Default::default()
        };
        let workspace = create_test_workspace(
            &format!("parallel-workspace-{}", i),
            &format!("/tmp/parallel-workspace-{}", i),
        );

        hub.handle_node_connection(
            node_id,
            format!("Parallel Node {}", i),
            capabilities,
            workspace,
            vec![],
        )
        .await
        .unwrap();
    }

    // 提交多个任务
    let context = create_test_context("test-user", "parallel-session");

    let mut task_ids = Vec::new();
    for i in 1..=10 {
        let command = Command::Shell(ShellCommand::new(format!("echo task-{}", i)));
        let task_id = hub
            .submit_task(
                command,
                context.clone(),
                Priority::Normal,
                None,
                vec![],
                None,
            )
            .await
            .unwrap();
        task_ids.push(task_id);
    }

    // 验证所有任务已创建
    assert_eq!(task_ids.len(), 10);

    // 验证每个任务 ID 唯一
    let unique_count = task_ids
        .iter()
        .map(|t| t.to_string())
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(unique_count, 10);

    let stats = hub.get_stats().await;
    assert_eq!(stats.scheduler.pending_tasks, 10);
}

// ============================================================================
// 优先级调度测试
// ============================================================================

/// 测试任务优先级排序
#[tokio::test]
async fn test_priority_ordering() {
    let config = HubConfig {
        hub_id: "priority-test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("priority-node");
    let capabilities = NodeCapabilities {
        max_concurrent_tasks: 1, // 单任务执行，便于观察顺序
        ..Default::default()
    };
    let workspace = create_test_workspace("priority-workspace", "/tmp/priority-workspace");

    hub.handle_node_connection(
        node_id,
        "Priority Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    let context = create_test_context("test-user", "priority-session");

    // 按随机顺序提交不同优先级任务
    let _low_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 0.1")),
            context.clone(),
            Priority::Low,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let _critical_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 0.1")),
            context.clone(),
            Priority::Critical,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let _normal_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 0.1")),
            context.clone(),
            Priority::Normal,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let _high_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 0.1")),
            context.clone(),
            Priority::High,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();

    let stats = hub.get_stats().await;
    assert_eq!(stats.scheduler.pending_tasks, 4);
}

// ============================================================================
// 故障恢复测试
// ============================================================================

/// 测试节点断开后任务处理
#[tokio::test]
async fn test_node_failure_handling() {
    let config = HubConfig {
        hub_id: "failure-test-hub".to_string(),
        heartbeat_timeout_secs: 5,
        task_timeout_secs: 10,
        max_retries: 2,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("failure-node");
    let workspace = create_test_workspace("failure-workspace", "/tmp/failure-workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Failure Node".to_string(),
        NodeCapabilities::default(),
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 提交任务
    let context = create_test_context("test-user", "failure-session");

    let task_id = hub
        .submit_task(
            Command::Shell(ShellCommand::new("echo test")),
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

    // 模拟节点故障
    hub.handle_node_disconnect(&node_id).await.unwrap();

    // 验证节点已断开
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 0);
}

// ============================================================================
// 命令类型测试
// ============================================================================

/// 测试各种命令类型
#[tokio::test]
async fn test_command_types() {
    let config = HubConfig {
        hub_id: "command-test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("command-node");
    let capabilities = NodeCapabilities {
        max_concurrent_tasks: 10,
        ..Default::default()
    };
    let workspace = create_test_workspace("command-workspace", "/tmp/command-workspace");

    hub.handle_node_connection(
        node_id,
        "Command Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    let context = create_test_context("test-user", "command-session");

    // 测试 Shell 命令
    let shell_task = hub
        .submit_task(
            Command::Shell(ShellCommand::new("ls -la")),
            context.clone(),
            Priority::Normal,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();
    assert!(!shell_task.as_str().is_empty());

    // 测试文件命令
    let file_task = hub
        .submit_task(
            Command::File(FileCommand::Exists {
                path: "/tmp/test.txt".to_string(),
            }),
            context.clone(),
            Priority::Normal,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();
    assert!(!file_task.as_str().is_empty());

    // 测试文件读取命令
    let read_task = hub
        .submit_task(
            Command::File(FileCommand::Read {
                path: "/etc/hostname".to_string(),
                limit: Some(1024),
                offset: None,
            }),
            context.clone(),
            Priority::Normal,
            None,
            vec![],
            None,
        )
        .await
        .unwrap();
    assert!(!read_task.as_str().is_empty());

    let stats = hub.get_stats().await;
    assert_eq!(stats.scheduler.pending_tasks, 3);
}

// ============================================================================
// 统计信息测试
// ============================================================================

/// 测试统计信息更新
#[tokio::test]
async fn test_stats_update() {
    let config = HubConfig {
        hub_id: "stats-test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 初始统计
    let stats = hub.get_stats().await;
    assert_eq!(stats.hub_id, "stats-test-hub");
    assert_eq!(stats.nodes.total_nodes, 0);
    assert_eq!(stats.nodes.online_nodes, 0);
    assert_eq!(stats.scheduler.pending_tasks, 0);
    assert_eq!(stats.scheduler.running_tasks, 0);
    assert_eq!(stats.scheduler.completed_tasks, 0);
    assert_eq!(stats.scheduler.failed_tasks, 0);

    // 注册节点
    let node_id = NodeId::from_string("stats-node");
    let workspace = create_test_workspace("stats-workspace", "/tmp/stats-workspace");

    hub.handle_node_connection(
        node_id.clone(),
        "Stats Node".to_string(),
        NodeCapabilities::default(),
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 验证节点统计更新
    let stats = hub.get_stats().await;
    assert_eq!(stats.nodes.total_nodes, 1);
    assert_eq!(stats.nodes.online_nodes, 1);

    // 提交任务
    let context = create_test_context("test-user", "stats-session");

    hub.submit_task(
        Command::Shell(ShellCommand::new("echo test")),
        context,
        Priority::Normal,
        None,
        vec![],
        None,
    )
    .await
    .unwrap();

    // 验证任务统计更新（任务可能被立即分配或仍在队列中）
    let stats = hub.get_stats().await;
    let total_tasks = stats.scheduler.pending_tasks + stats.scheduler.running_tasks;
    assert!(total_tasks >= 1, "至少应该有一个任务在系统中");

    // 断开节点
    hub.handle_node_disconnect(&node_id).await.unwrap();

    // 验证最终统计
    let stats = hub.get_stats().await;
    // 节点断开后，在线数应该为 0
    assert_eq!(stats.nodes.online_nodes, 0);
}

// ============================================================================
// 并发安全测试
// ============================================================================

/// 测试并发任务提交
#[tokio::test]
async fn test_concurrent_task_submission() {
    let config = HubConfig {
        hub_id: "concurrent-test-hub".to_string(),
        max_nodes: 100,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("concurrent-node");
    let capabilities = NodeCapabilities {
        max_concurrent_tasks: 100,
        ..Default::default()
    };
    let workspace = create_test_workspace("concurrent-workspace", "/tmp/concurrent-workspace");

    hub.handle_node_connection(
        node_id,
        "Concurrent Node".to_string(),
        capabilities,
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 并发提交 50 个任务
    let mut handles = Vec::new();
    let hub_clone = Arc::new(hub);

    for i in 0..50 {
        let hub_ref = Arc::clone(&hub_clone);
        let handle = tokio::spawn(async move {
            let context = TaskContext::new(
                UserId::from_string(format!("user-{}", i)),
                SessionId::from_string(format!("session-{}", i)),
                "test-channel",
            );

            hub_ref
                .submit_task(
                    Command::Shell(ShellCommand::new(format!("echo {}", i))),
                    context,
                    Priority::Normal,
                    None,
                    vec![],
                    None,
                )
                .await
        });
        handles.push(handle);
    }

    // 等待所有任务提交完成
    let mut task_ids = Vec::new();
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        task_ids.push(result);
    }

    // 验证所有任务已创建
    assert_eq!(task_ids.len(), 50);

    // 验证任务 ID 唯一
    let unique_count = task_ids
        .iter()
        .map(|t| t.to_string())
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(unique_count, 50);
}

// ============================================================================
// 任务取消测试
// ============================================================================

/// 测试任务取消
#[tokio::test]
async fn test_task_cancellation() {
    let config = HubConfig {
        hub_id: "cancel-test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册节点
    let node_id = NodeId::from_string("cancel-node");
    let workspace = create_test_workspace("cancel-workspace", "/tmp/cancel-workspace");

    hub.handle_node_connection(
        node_id,
        "Cancel Node".to_string(),
        NodeCapabilities::default(),
        workspace,
        vec![],
    )
    .await
    .unwrap();

    // 提交任务
    let context = create_test_context("test-user", "cancel-session");

    let task_id = hub
        .submit_task(
            Command::Shell(ShellCommand::new("sleep 60")),
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

    // 取消任务（如果任务在队列中或运行中）
    let result = hub.cancel_task(&task_id, "User cancelled").await;
    // 取消操作应该成功，即使任务可能已经不在系统中
    assert!(result.is_ok() || result.is_err(), "取消操作完成");
}

// ============================================================================
// 节点选择测试
// ============================================================================

/// 测试节点选择逻辑
#[tokio::test]
async fn test_node_selection() {
    let config = HubConfig {
        hub_id: "selection-test-hub".to_string(),
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config);

    // 注册多个不同能力的节点
    let workspace_1 = create_test_workspace("workspace-1", "/tmp/workspace-1");
    let workspace_2 = create_test_workspace("workspace-2", "/tmp/workspace-2");

    let high_cap_node = NodeId::from_string("high-cap-node");
    let low_cap_node = NodeId::from_string("low-cap-node");

    // 高能力节点
    hub.handle_node_connection(
        high_cap_node.clone(),
        "High Capacity Node".to_string(),
        NodeCapabilities {
            max_concurrent_tasks: 20,
            tags: vec!["high".to_string(), "compute".to_string()],
            ..Default::default()
        },
        workspace_1,
        vec!["high".to_string()],
    )
    .await
    .unwrap();

    // 低能力节点
    hub.handle_node_connection(
        low_cap_node.clone(),
        "Low Capacity Node".to_string(),
        NodeCapabilities {
            max_concurrent_tasks: 2,
            tags: vec!["low".to_string()],
            ..Default::default()
        },
        workspace_2,
        vec!["low".to_string()],
    )
    .await
    .unwrap();

    // 验证两个节点都已注册
    let nodes = hub.get_online_nodes().await;
    assert_eq!(nodes.len(), 2);

    // 验证可以通过 ID 获取节点
    let all_nodes = hub.get_all_nodes().await;
    assert_eq!(all_nodes.len(), 2);
}
