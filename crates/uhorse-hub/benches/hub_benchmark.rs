//! 性能基准测试
//!
//! 测试 Hub 的吞吐量和延迟

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use tokio::runtime::Runtime;
use uhorse_hub::{Hub, HubConfig};
use uhorse_protocol::{
    Command, NodeCapabilities, NodeId, Priority, ShellCommand,
    TaskContext, WorkspaceInfo, UserId, SessionId,
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
        "benchmark-channel",
    )
}

/// 设置测试环境：创建 Hub 并注册节点
fn setup_hub(node_count: usize) -> (Arc<Hub>, HubConfig) {
    let config = HubConfig {
        hub_id: "benchmark-hub".to_string(),
        max_nodes: node_count,
        ..Default::default()
    };

    let (hub, _rx) = Hub::new(config.clone());
    let hub = Arc::new(hub);

    // 注册节点
    for i in 0..node_count {
        let node_id = NodeId::from_string(&format!("bench-node-{}", i));
        let capabilities = NodeCapabilities {
            max_concurrent_tasks: 100,
            ..Default::default()
        };
        let workspace = create_test_workspace(
            &format!("bench-workspace-{}", i),
            &format!("/tmp/bench-workspace-{}", i),
        );

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            hub.handle_node_connection(
                node_id,
                format!("Benchmark Node {}", i),
                capabilities,
                workspace,
                vec![],
            )
            .await
            .unwrap();
        });
    }

    (hub, config)
}

/// 基准测试：任务提交吞吐量
fn bench_task_submission(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_submission");

    for node_count in [1, 5, 10].iter() {
        let (hub, _config) = setup_hub(*node_count);
        let context = create_test_context("bench-user", "bench-session");

        group.bench_with_input(
            BenchmarkId::new("nodes", node_count),
            node_count,
            |b, _| {
                let rt = Runtime::new().unwrap();
                b.to_async(&rt).iter(|| {
                    let hub = Arc::clone(&hub);
                    let ctx = context.clone();
                    async move {
                        hub.submit_task(
                            Command::Shell(ShellCommand::new("echo benchmark")),
                            ctx,
                            Priority::Normal,
                            None,
                            vec![],
                            None,
                        )
                        .await
                        .unwrap()
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：批量任务提交
fn bench_batch_task_submission(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_submission");
    group.sample_size(10);

    let (hub, _config) = setup_hub(10);

    for batch_size in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            batch_size,
            |b, &size| {
                let rt = Runtime::new().unwrap();
                b.to_async(&rt).iter(|| {
                    let hub = Arc::clone(&hub);
                    async move {
                        let mut tasks = Vec::with_capacity(size);
                        for i in 0..size {
                            let ctx = create_test_context(
                                &format!("user-{}", i),
                                &format!("session-{}", i),
                            );
                            let task = hub.submit_task(
                                Command::Shell(ShellCommand::new(&format!("echo {}", i))),
                                ctx,
                                Priority::Normal,
                                None,
                                vec![],
                                None,
                            )
                            .await
                            .unwrap();
                            tasks.push(task);
                        }
                        tasks
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：节点注册
fn bench_node_registration(c: &mut Criterion) {
    let config = HubConfig {
        hub_id: "bench-node-reg".to_string(),
        max_nodes: 1000,
        ..Default::default()
    };

    c.bench_function("node_registration", |b| {
        let rt = Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            async {
                let (hub, _rx) = Hub::new(config.clone());
                let node_id = NodeId::new();
                let workspace = create_test_workspace("bench-workspace", "/tmp/bench");

                hub.handle_node_connection(
                    node_id,
                    "Benchmark Node".to_string(),
                    NodeCapabilities::default(),
                    workspace,
                    vec![],
                )
                .await
                .unwrap();

                hub
            }
        });
    });
}

/// 基准测试：统计信息获取
fn bench_stats_retrieval(c: &mut Criterion) {
    let (hub, _config) = setup_hub(10);

    c.bench_function("stats_retrieval", |b| {
        let rt = Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            let hub = Arc::clone(&hub);
            async move { hub.get_stats().await }
        });
    });
}

/// 基准测试：节点查询
fn bench_node_lookup(c: &mut Criterion) {
    let (hub, _config) = setup_hub(100);

    c.bench_function("node_lookup", |b| {
        let rt = Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            let hub = Arc::clone(&hub);
            async move { hub.get_online_nodes().await }
        });
    });
}

/// 基准测试：并发任务提交
fn bench_concurrent_submission(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_submission");
    group.sample_size(10);

    let (hub, _config) = setup_hub(10);

    for concurrency in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrency", concurrency),
            concurrency,
            |b, &conc| {
                let rt = Runtime::new().unwrap();
                b.to_async(&rt).iter(|| {
                    let hub = Arc::clone(&hub);
                    async move {
                        let mut handles = Vec::with_capacity(conc);
                        for i in 0..conc {
                            let hub_clone = Arc::clone(&hub);
                            let handle = tokio::spawn(async move {
                                let ctx = create_test_context(
                                    &format!("user-{}", i),
                                    &format!("session-{}", i),
                                );
                                hub_clone
                                    .submit_task(
                                        Command::Shell(ShellCommand::new(&format!("echo {}", i))),
                                        ctx,
                                        Priority::Normal,
                                        None,
                                        vec![],
                                        None,
                                    )
                                    .await
                                    .unwrap()
                            });
                            handles.push(handle);
                        }

                        // 等待所有任务完成
                        let mut results = Vec::with_capacity(conc);
                        for handle in handles {
                            results.push(handle.await.unwrap());
                        }
                        results
                    }
                });
            },
        );
    }

    group.finish();
}

/// 基准测试：消息编解码
fn bench_message_encoding(c: &mut Criterion) {
    use uhorse_protocol::{HubToNode, MessageCodec, MessageId};
    use chrono::Utc;

    let mut group = c.benchmark_group("message_codec");

    let heartbeat = HubToNode::HeartbeatRequest {
        message_id: MessageId::new(),
        timestamp: Utc::now(),
    };

    group.bench_function("encode_heartbeat", |b| {
        b.iter(|| {
            MessageCodec::encode_hub_to_node(black_box(&heartbeat)).unwrap()
        });
    });

    let encoded = MessageCodec::encode_hub_to_node(&heartbeat).unwrap();

    group.bench_function("decode_heartbeat", |b| {
        b.iter(|| {
            MessageCodec::decode_hub_to_node(black_box(&encoded)).unwrap()
        });
    });

    group.finish();
}

/// 基准测试：优先级排序
fn bench_priority_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_sorting");

    let (hub, _config) = setup_hub(1);
    let context = create_test_context("bench-user", "bench-session");

    for task_count in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("tasks", task_count),
            task_count,
            |b, &count| {
                let rt = Runtime::new().unwrap();
                b.to_async(&rt).iter(|| {
                    let hub = Arc::clone(&hub);
                    let ctx = context.clone();
                    async move {
                        // 提交混合优先级的任务
                        for i in 0..count {
                            let priority = match i % 6 {
                                0 => Priority::Background,
                                1 => Priority::Low,
                                2 => Priority::Normal,
                                3 => Priority::High,
                                4 => Priority::Urgent,
                                _ => Priority::Critical,
                            };
                            hub.submit_task(
                                Command::Shell(ShellCommand::new(&format!("echo {}", i))),
                                ctx.clone(),
                                priority,
                                None,
                                vec![],
                                None,
                            )
                            .await
                            .unwrap();
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_task_submission,
    bench_batch_task_submission,
    bench_node_registration,
    bench_stats_retrieval,
    bench_node_lookup,
    bench_concurrent_submission,
    bench_message_encoding,
    bench_priority_sorting,
);

criterion_main!(benches);
