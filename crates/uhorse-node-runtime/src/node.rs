//! 本地节点
//!
//! 负责管理节点生命周期、处理任务和与 Hub 通信

use crate::connection::{ConnectionConfig, HubConnection};
use crate::error::{NodeError, NodeResult};
use crate::executor::CommandExecutor;
use crate::permission::{
    Action, ApprovalResolution, PermissionManager, PermissionResult, PermissionRule,
    ResourcePattern,
};
use crate::status::{HeartbeatSnapshot, Metrics, StatusReporter};
use crate::workspace::Workspace;
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{
    CommandResult, ErrorSource, ExecutionError, ExecutionMetrics, HubToNode, NodeCapabilities,
    NodeId, NodeStatus, NodeToHub, NotificationEvent, NotificationEventKind, TaskId,
};

/// 节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// 节点 ID（如果不设置则自动生成）
    #[serde(default)]
    pub node_id: Option<NodeId>,

    /// 节点名称
    pub name: String,

    /// Hub 连接配置
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// 工作空间路径
    pub workspace_path: String,

    /// 心跳间隔（秒）
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// 状态报告间隔（秒）
    #[serde(default = "default_status_interval")]
    pub status_interval_secs: u64,

    /// 最大并发任务数
    #[serde(default = "default_max_tasks")]
    pub max_concurrent_tasks: usize,

    /// 节点能力
    #[serde(default)]
    pub capabilities: NodeCapabilities,

    /// 标签（用于任务路由）
    #[serde(default)]
    pub tags: Vec<String>,

    /// 是否启用 git 保护
    #[serde(default = "default_git_protection_enabled")]
    pub git_protection_enabled: bool,

    /// 是否监听工作区变更
    #[serde(default = "default_watch_workspace")]
    pub watch_workspace: bool,

    /// 是否自动将新增文件加入 git
    #[serde(default = "default_auto_git_add_new_files")]
    pub auto_git_add_new_files: bool,

    /// 是否要求工作区必须是 git 仓库
    #[serde(default = "default_require_git_repo")]
    pub require_git_repo: bool,

    /// 内部工作目录
    #[serde(default = "default_internal_work_dir")]
    pub internal_work_dir: String,

    /// 额外权限规则
    #[serde(default)]
    pub permission_rules: Vec<PermissionRuleConfig>,
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_status_interval() -> u64 {
    60
}

fn default_max_tasks() -> usize {
    5
}

fn default_git_protection_enabled() -> bool {
    true
}

fn default_watch_workspace() -> bool {
    true
}

fn default_auto_git_add_new_files() -> bool {
    true
}

fn default_require_git_repo() -> bool {
    true
}

fn default_internal_work_dir() -> String {
    ".uhorse".to_string()
}

/// 权限规则配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleConfig {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 规则描述
    #[serde(default)]
    pub description: Option<String>,
    /// 资源配置
    pub resource: PermissionResourceConfig,
    /// 允许的操作
    pub actions: Vec<Action>,
    /// 是否需要审批
    #[serde(default)]
    pub require_approval: bool,
    /// 规则优先级
    #[serde(default)]
    pub priority: i32,
    /// 是否启用
    #[serde(default = "default_permission_rule_enabled")]
    pub enabled: bool,
}

/// 权限资源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PermissionResourceConfig {
    /// 允许所有资源
    AllowAll,
    /// 精确路径
    ExactPath {
        /// 路径
        path: String,
    },
    /// 路径前缀
    PathPrefix {
        /// 前缀
        prefix: String,
    },
    /// Glob 模式
    Glob {
        /// 模式
        pattern: String,
    },
    /// 正则表达式
    Regex {
        /// 表达式
        pattern: String,
    },
    /// 命令类型集合
    CommandType {
        /// 类型列表
        types: Vec<String>,
    },
    /// 全部匹配
    All {
        /// 子模式列表
        patterns: Vec<PermissionResourceConfig>,
    },
    /// 任一匹配
    Any {
        /// 子模式列表
        patterns: Vec<PermissionResourceConfig>,
    },
}

fn default_permission_rule_enabled() -> bool {
    true
}

impl PermissionResourceConfig {
    fn into_runtime(self) -> ResourcePattern {
        match self {
            Self::AllowAll => ResourcePattern::AllowAll,
            Self::ExactPath { path } => ResourcePattern::ExactPath { path },
            Self::PathPrefix { prefix } => ResourcePattern::PathPrefix { prefix },
            Self::Glob { pattern } => ResourcePattern::Glob { pattern },
            Self::Regex { pattern } => ResourcePattern::Regex { pattern },
            Self::CommandType { types } => ResourcePattern::CommandType { types },
            Self::All { patterns } => ResourcePattern::All {
                patterns: patterns.into_iter().map(Self::into_runtime).collect(),
            },
            Self::Any { patterns } => ResourcePattern::Any {
                patterns: patterns.into_iter().map(Self::into_runtime).collect(),
            },
        }
    }
}

impl PermissionRuleConfig {
    fn into_runtime(self) -> PermissionRule {
        let mut rule = PermissionRule::new(self.id, self.name)
            .with_resource(self.resource.into_runtime())
            .with_actions(self.actions)
            .require_approval(self.require_approval)
            .with_priority(self.priority);
        rule.description = self.description;
        rule.enabled = self.enabled;
        rule
    }
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            name: "uHorse-Node".to_string(),
            connection: ConnectionConfig::default(),
            workspace_path: ".".to_string(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            status_interval_secs: default_status_interval(),
            max_concurrent_tasks: default_max_tasks(),
            capabilities: NodeCapabilities::default(),
            tags: vec!["default".to_string()],
            git_protection_enabled: default_git_protection_enabled(),
            watch_workspace: default_watch_workspace(),
            auto_git_add_new_files: default_auto_git_add_new_files(),
            require_git_repo: default_require_git_repo(),
            internal_work_dir: default_internal_work_dir(),
            permission_rules: vec![],
        }
    }
}

/// 本地节点
pub struct Node {
    /// 配置
    config: NodeConfig,

    /// 节点 ID
    node_id: NodeId,

    /// 工作空间
    workspace: Arc<Workspace>,

    /// Hub 连接
    connection: HubConnection,

    /// 命令执行器
    executor: Arc<CommandExecutor>,

    /// 权限管理器
    permission_manager: Arc<PermissionManager>,

    /// 状态报告器
    status_reporter: Arc<StatusReporter>,

    /// 指标收集
    metrics: Arc<RwLock<Metrics>>,

    /// 运行任务
    running_tasks: Arc<RwLock<HashMap<TaskId, RunningTask>>>,

    /// 运行标志
    running: Arc<AtomicBool>,

    /// 停止信号
    stop_signal: broadcast::Sender<()>,

    /// 状态更新广播
    status_tx: broadcast::Sender<NodeStatus>,

    /// 最新心跳快照
    heartbeat_snapshot: Arc<RwLock<Option<HeartbeatSnapshot>>>,

    /// 工作区 watcher
    workspace_watcher: Option<RecommendedWatcher>,

    /// 业务出站消息发送器
    outbound_tx: Option<mpsc::Sender<NodeToHub>>,
}

/// 运行中的任务
#[derive(Debug)]
struct RunningTask {
    /// 取消信号
    cancel_tx: mpsc::Sender<()>,
}

fn convert_protocol_permission_rule(rule: uhorse_protocol::PermissionRule) -> PermissionRule {
    let mut runtime_rule = PermissionRule::new(rule.id, rule.name)
        .with_resource(convert_protocol_resource_pattern(rule.resource))
        .with_actions(
            rule.actions
                .into_iter()
                .filter_map(convert_protocol_action)
                .collect(),
        )
        .require_approval(rule.require_approval);
    runtime_rule.enabled = rule.enabled;
    runtime_rule
}

fn convert_protocol_resource_pattern(pattern: uhorse_protocol::ResourcePattern) -> ResourcePattern {
    match pattern {
        uhorse_protocol::ResourcePattern::Exact { path } => ResourcePattern::ExactPath { path },
        uhorse_protocol::ResourcePattern::Glob { pattern } => ResourcePattern::Glob { pattern },
        uhorse_protocol::ResourcePattern::Regex { pattern } => ResourcePattern::Regex { pattern },
        uhorse_protocol::ResourcePattern::Prefix { prefix } => {
            ResourcePattern::PathPrefix { prefix }
        }
    }
}

fn convert_protocol_action(action: uhorse_protocol::Action) -> Option<Action> {
    match action {
        uhorse_protocol::Action::Read => Some(Action::Read),
        uhorse_protocol::Action::Write => Some(Action::Write),
        uhorse_protocol::Action::Delete => Some(Action::Delete),
        uhorse_protocol::Action::Execute => Some(Action::Execute),
        uhorse_protocol::Action::Admin => Some(Action::Admin),
        uhorse_protocol::Action::List => None,
    }
}

impl Node {
    /// 创建新的节点
    pub fn new(config: NodeConfig) -> NodeResult<Self> {
        // 创建工作空间
        let workspace = Arc::new(Workspace::new(&config.workspace_path)?);

        if config.require_git_repo && !workspace.is_git_repo() {
            return Err(NodeError::Config(format!(
                "Workspace must be a git repository when require_git_repo is enabled: {}",
                workspace.root().display()
            )));
        }

        // 创建权限管理器
        let permission_manager = Arc::new(PermissionManager::new(
            workspace.clone(),
            config.git_protection_enabled,
        ));

        // 创建执行器
        let executor = Arc::new(CommandExecutor::new(
            workspace.clone(),
            permission_manager.clone(),
            config.internal_work_dir.clone(),
        ));

        // 创建节点 ID
        let node_id = config.node_id.clone().unwrap_or_default();

        // 创建状态报告器
        let status_reporter = Arc::new(
            StatusReporter::new(node_id.clone()).with_interval(config.status_interval_secs),
        );

        // 创建广播通道
        let (stop_signal, _) = broadcast::channel(1);
        let (status_tx, _) = broadcast::channel(16);
        let heartbeat_snapshot = Arc::new(RwLock::new(None));

        // 创建连接
        let connection = HubConnection::new(
            node_id.clone(),
            config.connection.clone(),
            config.name.clone(),
            workspace.root().to_string_lossy().to_string(),
            config.capabilities.clone(),
            heartbeat_snapshot.clone(),
        );

        Ok(Self {
            config,
            node_id,
            workspace,
            connection,
            executor,
            permission_manager,
            status_reporter,
            metrics: Arc::new(RwLock::new(Metrics::default())),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            stop_signal,
            status_tx,
            heartbeat_snapshot,
            workspace_watcher: None,
            outbound_tx: None,
        })
    }

    /// 启动节点
    pub async fn start(&mut self) -> NodeResult<()> {
        if self.running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(NodeError::Internal("Node already running".to_string()));
        }

        info!("Starting node: {} ({})", self.config.name, self.node_id);

        // 1. 加载默认权限规则
        self.permission_manager.load_default_rules().await;
        for rule in self.config.permission_rules.clone() {
            self.permission_manager.add_rule(rule.into_runtime()).await;
        }

        // 2. 连接到 Hub
        let (hub_rx, outbound_tx) = self.connection.start().await?;

        if self.config.watch_workspace {
            if let Err(error) = self.start_workspace_watcher() {
                self.running.store(false, Ordering::SeqCst);
                self.connection.stop().await;
                return Err(error);
            }
        }

        // 3. 启动后台任务
        self.outbound_tx = Some(outbound_tx.clone());
        self.start_status_task();
        self.start_message_handler(hub_rx, outbound_tx);

        info!("Node started successfully");
        Ok(())
    }

    fn start_workspace_watcher(&mut self) -> NodeResult<()> {
        let root = self.workspace.root().to_path_buf();
        let internal_work_dir = self.config.internal_work_dir.clone();
        let auto_git_add_new_files = self.config.auto_git_add_new_files;
        let running = self.running.clone();
        let (tx, rx) = std_mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |result| {
                let _ = tx.send(result);
            },
            NotifyConfig::default(),
        )
        .map_err(|e| NodeError::Execution(format!("Failed to create workspace watcher: {}", e)))?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| {
                NodeError::Execution(format!("Failed to watch workspace {:?}: {}", root, e))
            })?;

        let watcher_root = root.clone();
        tokio::task::spawn_blocking(move || {
            while running.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(Ok(event)) => {
                        if let Err(error) = Self::handle_workspace_event(
                            &watcher_root,
                            &internal_work_dir,
                            auto_git_add_new_files,
                            event,
                        ) {
                            warn!("Failed to handle workspace event: {}", error);
                        }
                    }
                    Ok(Err(error)) => {
                        warn!("Workspace watcher error: {}", error);
                    }
                    Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        self.workspace_watcher = Some(watcher);
        info!("Workspace watcher started for {}", root.display());
        Ok(())
    }

    fn handle_workspace_event(
        root: &Path,
        internal_work_dir: &str,
        auto_git_add_new_files: bool,
        event: Event,
    ) -> NodeResult<()> {
        if !auto_git_add_new_files || !matches!(event.kind, EventKind::Create(_)) {
            return Ok(());
        }

        for path in event.paths {
            if Self::should_skip_watched_path(root, internal_work_dir, &path) {
                continue;
            }

            if path.is_dir() {
                continue;
            }

            Self::git_add_path(root, &path)?;
        }

        Ok(())
    }

    fn should_skip_watched_path(root: &Path, internal_work_dir: &str, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return true;
        };

        let mut components = relative.components();
        let Some(first) = components.next() else {
            return true;
        };

        let first = first.as_os_str().to_string_lossy();
        first == ".git" || first == internal_work_dir
    }

    fn git_add_path(root: &Path, path: &Path) -> NodeResult<()> {
        let relative = path.strip_prefix(root).map_err(|_| {
            NodeError::Execution(format!(
                "Failed to derive relative path for watched file: {}",
                path.display()
            ))
        })?;

        let output = StdCommand::new("git")
            .arg("-C")
            .arg(root)
            .arg("add")
            .arg("--")
            .arg(relative)
            .output()
            .map_err(|e| NodeError::Execution(format!("Failed to run git add: {}", e)))?;

        if !output.status.success() {
            return Err(NodeError::Execution(format!(
                "git add failed for {}: {}",
                relative.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        info!("Auto staged new file: {}", relative.display());
        Ok(())
    }

    /// 启动状态报告任务
    fn start_status_task(&self) {
        let interval_secs = self.config.status_interval_secs;
        let running = self.running.clone();
        let status_tx = self.status_tx.clone();
        let heartbeat_snapshot = self.heartbeat_snapshot.clone();
        let status_reporter = self.status_reporter.clone();
        let running_tasks = self.running_tasks.clone();
        let max_concurrent_tasks = self.config.max_concurrent_tasks;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                if !running.load(Ordering::SeqCst) {
                    break;
                }

                let current_tasks = running_tasks.read().await.len();
                match Self::collect_heartbeat_snapshot(
                    &status_reporter,
                    current_tasks,
                    max_concurrent_tasks,
                    None,
                )
                .await
                {
                    Ok(snapshot) => {
                        {
                            let mut latest_snapshot = heartbeat_snapshot.write().await;
                            *latest_snapshot = Some(snapshot.clone());
                        }
                        if let Err(error) = status_tx.send(snapshot.status.clone()) {
                            debug!("No active status subscribers: {}", error);
                        }
                    }
                    Err(error) => {
                        warn!("Failed to collect node status: {}", error);
                    }
                }
            }
        });
    }

    async fn collect_heartbeat_snapshot(
        status_reporter: &Arc<StatusReporter>,
        current_tasks: usize,
        max_tasks: usize,
        network_latency_ms: Option<u64>,
    ) -> NodeResult<HeartbeatSnapshot> {
        status_reporter
            .collect_snapshot(current_tasks, max_tasks, network_latency_ms)
            .await
            .map_err(|error| {
                NodeError::Internal(format!("Failed to collect heartbeat snapshot: {}", error))
            })
    }

    /// 启动消息处理器
    fn start_message_handler(
        &self,
        mut receiver: mpsc::Receiver<HubToNode>,
        outbound_tx: mpsc::Sender<NodeToHub>,
    ) {
        let running = self.running.clone();
        let executor = self.executor.clone();
        let permission_manager = self.permission_manager.clone();
        let metrics = self.metrics.clone();
        let running_tasks = self.running_tasks.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            loop {
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                tokio::select! {
                    // 接收 Hub 消息
                    msg = receiver.recv() => {
                        match msg {
                            Some(message) => {
                                if let Err(e) = Self::handle_hub_message(
                                    &node_id,
                                    &message,
                                    &executor,
                                    &permission_manager,
                                    &metrics,
                                    &running_tasks,
                                    &outbound_tx,
                                ).await {
                                    error!("Failed to handle Hub message: {}", e);
                                }
                            }
                            None => {
                                info!("Hub connection closed");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    /// 处理 Hub 消息
    async fn handle_hub_message(
        node_id: &NodeId,
        message: &HubToNode,
        executor: &Arc<CommandExecutor>,
        permission_manager: &Arc<PermissionManager>,
        metrics: &Arc<RwLock<Metrics>>,
        running_tasks: &Arc<RwLock<HashMap<TaskId, RunningTask>>>,
        outbound_tx: &mpsc::Sender<NodeToHub>,
    ) -> NodeResult<()> {
        match message {
            HubToNode::TaskAssignment {
                task_id,
                command,
                context,
                ..
            } => {
                info!("Received task: {}", task_id);
                let started_at = Instant::now();

                let (cancel_tx, _cancel_rx) = mpsc::channel(1);
                {
                    let mut tasks = running_tasks.write().await;
                    tasks.insert(task_id.clone(), RunningTask { cancel_tx });
                }

                match permission_manager.check(command, context).await {
                    PermissionResult::Allowed => {
                        let execution = executor.execute_unchecked(command).await;
                        let duration_ms = started_at.elapsed().as_millis() as u64;

                        let result = match execution {
                            Ok(result) => result,
                            Err(error) => Self::command_result_from_node_error(&error, duration_ms),
                        };

                        {
                            let mut m = metrics.write().await;
                            m.record_execution(result.success, duration_ms);
                        }

                        {
                            let mut tasks = running_tasks.write().await;
                            tasks.remove(task_id);
                        }

                        let message = NodeToHub::TaskResult {
                            message_id: uhorse_protocol::MessageId::new(),
                            task_id: task_id.clone(),
                            result: result.clone(),
                            metrics: ExecutionMetrics {
                                duration_ms,
                                ..Default::default()
                            },
                        };

                        outbound_tx.send(message).await.map_err(|e| {
                            NodeError::Connection(format!("Failed to send task result: {}", e))
                        })?;

                        info!(
                            "Task {} completed on node {}: {}",
                            task_id,
                            node_id,
                            if result.success { "success" } else { "failed" }
                        );
                    }
                    PermissionResult::Denied(reason) => {
                        let duration_ms = started_at.elapsed().as_millis() as u64;
                        let result =
                            CommandResult::failure(ExecutionError::permission_denied(&reason))
                                .with_duration(duration_ms);

                        {
                            let mut m = metrics.write().await;
                            m.record_execution(false, duration_ms);
                        }

                        {
                            let mut tasks = running_tasks.write().await;
                            tasks.remove(task_id);
                        }

                        outbound_tx
                            .send(NodeToHub::TaskResult {
                                message_id: uhorse_protocol::MessageId::new(),
                                task_id: task_id.clone(),
                                result,
                                metrics: ExecutionMetrics {
                                    duration_ms,
                                    ..Default::default()
                                },
                            })
                            .await
                            .map_err(|e| {
                                NodeError::Connection(format!("Failed to send task result: {}", e))
                            })?;

                        warn!("Task {} denied on node {}: {}", task_id, node_id, reason);
                    }
                    PermissionResult::RequiresApproval { reason, .. } => {
                        let approval = permission_manager
                            .create_approval_request(
                                task_id.as_str().to_string(),
                                command.clone(),
                                context.clone(),
                                reason.clone(),
                            )
                            .await?;

                        outbound_tx
                            .send(NodeToHub::ApprovalRequest {
                                message_id: uhorse_protocol::MessageId::new(),
                                request_id: approval.id.clone(),
                                task_id: task_id.clone(),
                                command: command.clone(),
                                context: context.clone(),
                                reason: approval.reason.clone(),
                                timestamp: approval.requested_at,
                                expires_at: approval.expires_at,
                            })
                            .await
                            .map_err(|e| {
                                NodeError::Connection(format!(
                                    "Failed to send approval request: {}",
                                    e
                                ))
                            })?;

                        info!(
                            "Task {} on node {} is waiting for approval {}",
                            task_id, node_id, approval.id
                        );
                    }
                }
            }

            HubToNode::TaskCancellation {
                task_id, reason, ..
            } => {
                info!("Task cancelled: {} - {}", task_id, reason);

                // 取消运行中的任务
                let mut tasks = running_tasks.write().await;
                if let Some(task) = tasks.remove(task_id) {
                    let _ = task.cancel_tx.send(()).await;
                }
            }

            HubToNode::HeartbeatRequest { .. } => {
                debug!("Heartbeat request received");
            }

            HubToNode::ConfigUpdate { .. } => {
                info!("Config update received");
            }

            HubToNode::PermissionUpdate { rules, .. } => {
                let runtime_rules = rules
                    .iter()
                    .cloned()
                    .map(convert_protocol_permission_rule)
                    .collect();
                permission_manager.replace_rules(runtime_rules).await;
                info!("Permission update applied: {} rules", rules.len());
            }

            HubToNode::SkillDeploy { .. } => {
                info!("Skill deploy received");
            }

            HubToNode::SkillRemove { .. } => {
                info!("Skill remove received");
            }

            HubToNode::ApprovalResponse {
                request_id,
                approved,
                responder,
                reason,
                ..
            } => {
                let resolution = permission_manager
                    .handle_approval_response(
                        request_id,
                        *approved,
                        responder.clone(),
                        reason.clone(),
                    )
                    .await?;

                match resolution {
                    ApprovalResolution::Approved { request, task_id } => {
                        info!(
                            "Approval response applied on node {}: request={}, approved=true",
                            node_id, request_id
                        );

                        let (cancel_tx, _cancel_rx) = mpsc::channel(1);
                        {
                            let mut tasks = running_tasks.write().await;
                            tasks.insert(task_id.clone(), RunningTask { cancel_tx });
                        }

                        let started_at = Instant::now();
                        let execution = executor.execute_unchecked(&request.command).await;
                        let duration_ms = started_at.elapsed().as_millis() as u64;

                        let result = match execution {
                            Ok(result) => result,
                            Err(error) => Self::command_result_from_node_error(&error, duration_ms),
                        };

                        {
                            let mut m = metrics.write().await;
                            m.record_execution(result.success, duration_ms);
                        }

                        {
                            let mut tasks = running_tasks.write().await;
                            tasks.remove(&task_id);
                        }

                        outbound_tx
                            .send(NodeToHub::TaskResult {
                                message_id: uhorse_protocol::MessageId::new(),
                                task_id: task_id.clone(),
                                result: result.clone(),
                                metrics: ExecutionMetrics {
                                    duration_ms,
                                    ..Default::default()
                                },
                            })
                            .await
                            .map_err(|e| {
                                NodeError::Connection(format!("Failed to send task result: {}", e))
                            })?;

                        info!(
                            "Approved task {} completed on node {}: {}",
                            task_id,
                            node_id,
                            if result.success { "success" } else { "failed" }
                        );
                    }
                    ApprovalResolution::Rejected { task_id, .. } => {
                        info!(
                            "Approval response applied on node {}: request={}, approved=false",
                            node_id, request_id
                        );

                        {
                            let mut tasks = running_tasks.write().await;
                            tasks.remove(&task_id);
                        }

                        {
                            let mut m = metrics.write().await;
                            m.record_execution(false, 0);
                        }

                        let result = CommandResult::failure(ExecutionError::permission_denied(
                            reason
                                .clone()
                                .unwrap_or_else(|| "Approval rejected".to_string()),
                        ));

                        outbound_tx
                            .send(NodeToHub::TaskResult {
                                message_id: uhorse_protocol::MessageId::new(),
                                task_id: task_id.clone(),
                                result,
                                metrics: ExecutionMetrics::default(),
                            })
                            .await
                            .map_err(|e| {
                                NodeError::Connection(format!("Failed to send task result: {}", e))
                            })?;

                        warn!("Approval rejected for task {} on node {}", task_id, node_id);
                    }
                }
            }

            HubToNode::Disconnect { reason, .. } => {
                warn!("Hub requested disconnect: {}", reason);
            }
        }

        Ok(())
    }

    fn command_result_from_node_error(error: &NodeError, duration_ms: u64) -> CommandResult {
        let execution_error = match error {
            NodeError::Permission(message) => ExecutionError::permission_denied(message.clone()),
            NodeError::Timeout(message) => ExecutionError::timeout(message.clone()),
            NodeError::Execution(message) => ExecutionError::execution_failed(message.clone()),
            NodeError::Workspace(message) => ExecutionError::validation_failed(message.clone()),
            NodeError::Config(message) => ExecutionError::validation_failed(message.clone()),
            NodeError::Connection(message) => {
                ExecutionError::new("CONNECTION_FAILED", message.clone(), ErrorSource::External)
            }
            NodeError::Protocol(error) => {
                ExecutionError::new("PROTOCOL_ERROR", error.to_string(), ErrorSource::Internal)
            }
            NodeError::Io(error) => {
                ExecutionError::new("IO_ERROR", error.to_string(), ErrorSource::Executor)
            }
            NodeError::Serialization(error) => ExecutionError::new(
                "SERIALIZATION_ERROR",
                error.to_string(),
                ErrorSource::Internal,
            ),
            NodeError::Internal(message) => {
                ExecutionError::new("INTERNAL_ERROR", message.clone(), ErrorSource::Internal)
            }
        };

        CommandResult::failure(execution_error).with_duration(duration_ms)
    }

    /// 立即上报通知事件到 Hub
    pub fn report_notification_nowait(&self, event: NotificationEvent) -> NodeResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(NodeError::Connection("Node is not running".to_string()));
        }

        let outbound_tx = self.outbound_tx.as_ref().ok_or_else(|| {
            NodeError::Connection("Outbound channel is not available".to_string())
        })?;

        outbound_tx
            .try_send(NodeToHub::NotificationEvent {
                message_id: uhorse_protocol::MessageId::new(),
                node_id: self.node_id.clone(),
                event,
            })
            .map_err(|error| {
                NodeError::Connection(format!("Failed to send notification event: {}", error))
            })
    }

    /// 以简化参数立即上报通知事件到 Hub
    pub fn report_notification_event_nowait(
        &self,
        kind: NotificationEventKind,
        title: impl Into<String>,
        body: impl Into<String>,
        details_included: bool,
    ) -> NodeResult<()> {
        self.report_notification_nowait(NotificationEvent::new(kind, title, body, details_included))
    }

    /// 停止节点
    pub async fn stop(&mut self) -> NodeResult<()> {
        if !self
            .running
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(()); // 已经停止
        }

        info!("Stopping node: {}", self.node_id);

        // 发送停止信号
        let _ = self.stop_signal.send(());

        self.workspace_watcher = None;
        self.outbound_tx = None;

        // 停止连接
        self.connection.stop().await;

        info!("Node stopped");
        Ok(())
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// 获取配置
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// 获取工作空间
    pub fn workspace(&self) -> Arc<Workspace> {
        self.workspace.clone()
    }

    /// 获取运行状态
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// 获取连接状态
    pub async fn connection_state(&self) -> crate::connection::ConnectionState {
        self.connection.state().await
    }

    /// 获取待审批数量
    pub async fn pending_approvals_count(&self) -> usize {
        self.permission_manager.get_pending_approvals().await.len()
    }

    /// 获取运行任务数量
    pub async fn running_tasks_count(&self) -> usize {
        self.running_tasks.read().await.len()
    }

    /// 获取最近一次心跳快照
    pub async fn heartbeat_snapshot(&self) -> Option<HeartbeatSnapshot> {
        self.heartbeat_snapshot.read().await.clone()
    }

    /// 获取状态流
    pub fn subscribe_status(&self) -> broadcast::Receiver<NodeStatus> {
        self.status_tx.subscribe()
    }

    /// 获取指标
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) {
            warn!("Node dropped while still running");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use notify::event::{CreateKind, EventAttributes, ModifyKind};
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uhorse_protocol::{Command, MessageId, TaskContext};

    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();
        assert_eq!(config.name, "uHorse-Node");
        assert_eq!(config.max_concurrent_tasks, 5);
        assert!(config.git_protection_enabled);
        assert!(config.watch_workspace);
        assert!(config.auto_git_add_new_files);
        assert!(config.require_git_repo);
        assert_eq!(config.internal_work_dir, ".uhorse");
        assert!(config.permission_rules.is_empty());
    }

    #[test]
    fn test_permission_rule_config_into_runtime_preserves_fields() {
        let rule = PermissionRuleConfig {
            id: "approval-shell".to_string(),
            name: "Require shell approval".to_string(),
            description: Some("shell needs approval".to_string()),
            resource: PermissionResourceConfig::CommandType {
                types: vec!["shell".to_string()],
            },
            actions: vec![Action::Execute],
            require_approval: true,
            priority: 9,
            enabled: false,
        };

        let runtime = rule.into_runtime();
        assert_eq!(runtime.id, "approval-shell");
        assert_eq!(runtime.name, "Require shell approval");
        assert_eq!(runtime.description.as_deref(), Some("shell needs approval"));
        assert_eq!(runtime.actions, vec![Action::Execute]);
        assert!(runtime.require_approval);
        assert_eq!(runtime.priority, 9);
        assert!(!runtime.enabled);
        match runtime.resource {
            ResourcePattern::CommandType { types } => assert_eq!(types, vec!["shell"]),
            other => panic!("unexpected resource: {:?}", other),
        }
    }

    #[test]
    fn test_convert_protocol_permission_rule_maps_execute_prefix_rule() {
        let runtime = convert_protocol_permission_rule(uhorse_protocol::PermissionRule {
            id: "rule-1".to_string(),
            name: "prefix".to_string(),
            resource: uhorse_protocol::ResourcePattern::Prefix {
                prefix: "/tmp".to_string(),
            },
            actions: vec![uhorse_protocol::Action::Execute],
            conditions: vec![],
            require_approval: true,
            enabled: true,
        });

        assert_eq!(runtime.id, "rule-1");
        assert_eq!(runtime.name, "prefix");
        assert_eq!(runtime.actions, vec![Action::Execute]);
        assert!(runtime.require_approval);
        assert!(runtime.enabled);
        match runtime.resource {
            ResourcePattern::PathPrefix { prefix } => assert_eq!(prefix, "/tmp"),
            other => panic!("unexpected resource: {:?}", other),
        }
    }

    #[test]
    fn test_node_new_requires_git_repo_when_enabled() {
        let temp = TempDir::new().unwrap();
        let result = Node::new(NodeConfig {
            workspace_path: temp.path().to_string_lossy().to_string(),
            ..Default::default()
        });

        match result {
            Err(NodeError::Config(_)) => {}
            other => panic!("unexpected result: {:?}", other.err()),
        }
    }

    #[test]
    fn test_node_new_allows_non_git_workspace_when_disabled() {
        let temp = TempDir::new().unwrap();
        let node = Node::new(NodeConfig {
            workspace_path: temp.path().to_string_lossy().to_string(),
            require_git_repo: false,
            ..Default::default()
        });

        assert!(node.is_ok());
    }

    #[test]
    fn test_workspace_watcher_skips_internal_and_git_paths() {
        let root = PathBuf::from("/tmp/workspace");

        assert!(Node::should_skip_watched_path(
            &root,
            ".uhorse",
            &root.join(".uhorse/script.py")
        ));
        assert!(Node::should_skip_watched_path(
            &root,
            ".uhorse",
            &root.join(".git/index")
        ));
        assert!(!Node::should_skip_watched_path(
            &root,
            ".uhorse",
            &root.join("src/main.rs")
        ));
    }

    #[test]
    fn test_handle_workspace_event_ignores_non_create_events() {
        let root = PathBuf::from("/tmp/workspace");
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: vec![root.join("new.txt")],
            attrs: EventAttributes::default(),
        };

        let result = Node::handle_workspace_event(&root, ".uhorse", true, event);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_workspace_event_skips_internal_dir_without_git() {
        let root = PathBuf::from("/tmp/workspace");
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![root.join(".uhorse/code.py")],
            attrs: EventAttributes::default(),
        };

        let result = Node::handle_workspace_event(&root, ".uhorse", true, event);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_hub_message_sends_approval_request_for_gated_task() {
        let temp = TempDir::new().unwrap();
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone(), true));
        permission_manager
            .add_rule(
                crate::permission::PermissionRule::new("approval-shell", "Require shell approval")
                    .with_resource(crate::permission::ResourcePattern::CommandType {
                        types: vec!["shell".to_string()],
                    })
                    .with_actions(vec![crate::permission::Action::Execute])
                    .require_approval(true)
                    .with_priority(10),
            )
            .await;
        let metrics = Arc::new(RwLock::new(Metrics::default()));
        let running_tasks = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(2);
        let task_id = TaskId::from_string("task-approval");
        let command = Command::Shell(
            uhorse_protocol::ShellCommand::new("sh")
                .with_args(vec!["-c".to_string(), "printf approved".to_string()]),
        );
        let context = TaskContext::new(
            uhorse_protocol::UserId::from_string("test-user"),
            uhorse_protocol::SessionId::new(),
            "test-channel",
        );

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::TaskAssignment {
                message_id: MessageId::new(),
                task_id: task_id.clone(),
                command: command.clone(),
                priority: uhorse_protocol::Priority::Normal,
                deadline: None,
                context: context.clone(),
                retry_count: 0,
                max_retries: 3,
            },
            &Arc::new(CommandExecutor::new(
                workspace,
                permission_manager.clone(),
                ".uhorse".to_string(),
            )),
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let message = tokio::time::timeout(Duration::from_secs(1), outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match message {
            NodeToHub::ApprovalRequest {
                task_id: approval_task_id,
                command: approval_command,
                context: approval_context,
                reason,
                ..
            } => {
                assert_eq!(approval_task_id, task_id);
                match approval_command {
                    Command::Shell(shell) => {
                        assert_eq!(shell.command, "sh");
                        assert_eq!(shell.args, vec!["-c", "printf approved"]);
                    }
                    other => panic!("unexpected command: {:?}", other),
                }
                assert_eq!(approval_context.session_id, context.session_id);
                assert!(reason.contains("Require shell approval"));
            }
            other => panic!("unexpected message: {:?}", other),
        }

        assert_eq!(permission_manager.get_pending_approvals().await.len(), 1);
        assert_eq!(running_tasks.read().await.len(), 1);
        assert_eq!(metrics.read().await.total_executions, 0);
    }

    #[tokio::test]
    async fn test_handle_hub_message_executes_task_after_approval() {
        let temp = TempDir::new().unwrap();
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone(), true));
        permission_manager
            .add_rule(
                crate::permission::PermissionRule::new("approval-shell", "Require shell approval")
                    .with_resource(crate::permission::ResourcePattern::CommandType {
                        types: vec!["shell".to_string()],
                    })
                    .with_actions(vec![crate::permission::Action::Execute])
                    .require_approval(true)
                    .with_priority(10),
            )
            .await;
        let metrics = Arc::new(RwLock::new(Metrics::default()));
        let running_tasks = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(4);
        let task_id = TaskId::from_string("task-approval");
        let command = Command::Shell(
            uhorse_protocol::ShellCommand::new("sh")
                .with_args(vec!["-c".to_string(), "printf approved".to_string()]),
        );
        let context = TaskContext::new(
            uhorse_protocol::UserId::from_string("test-user"),
            uhorse_protocol::SessionId::new(),
            "test-channel",
        );
        let executor = Arc::new(CommandExecutor::new(
            workspace,
            permission_manager.clone(),
            ".uhorse".to_string(),
        ));

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::TaskAssignment {
                message_id: MessageId::new(),
                task_id: task_id.clone(),
                command: command.clone(),
                priority: uhorse_protocol::Priority::Normal,
                deadline: None,
                context: context.clone(),
                retry_count: 0,
                max_retries: 3,
            },
            &executor,
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let approval_request = tokio::time::timeout(Duration::from_secs(1), outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();

        let request_id = match approval_request {
            NodeToHub::ApprovalRequest { request_id, .. } => request_id,
            other => panic!("unexpected message: {:?}", other),
        };

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::ApprovalResponse {
                message_id: MessageId::new(),
                request_id,
                approved: true,
                responder: "admin".to_string(),
                reason: Some("approved".to_string()),
                responded_at: Utc::now(),
            },
            &executor,
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let message = tokio::time::timeout(Duration::from_secs(1), outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match message {
            NodeToHub::TaskResult {
                task_id: result_task_id,
                result,
                ..
            } => {
                assert_eq!(result_task_id, task_id);
                assert!(result.success);
                assert_eq!(result.output.as_text(), Some("approved"));
            }
            other => panic!("unexpected message: {:?}", other),
        }

        assert!(permission_manager.get_pending_approvals().await.is_empty());
        assert!(running_tasks.read().await.is_empty());
        assert_eq!(metrics.read().await.total_executions, 1);
        assert_eq!(metrics.read().await.successful_executions, 1);
    }

    #[tokio::test]
    async fn test_handle_hub_message_reports_rejected_approval_as_task_failure() {
        let temp = TempDir::new().unwrap();
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone(), true));
        permission_manager
            .add_rule(
                crate::permission::PermissionRule::new("approval-shell", "Require shell approval")
                    .with_resource(crate::permission::ResourcePattern::CommandType {
                        types: vec!["shell".to_string()],
                    })
                    .with_actions(vec![crate::permission::Action::Execute])
                    .require_approval(true)
                    .with_priority(10),
            )
            .await;
        let metrics = Arc::new(RwLock::new(Metrics::default()));
        let running_tasks = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(4);
        let task_id = TaskId::from_string("task-approval");
        let executor = Arc::new(CommandExecutor::new(
            workspace,
            permission_manager.clone(),
            ".uhorse".to_string(),
        ));

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::TaskAssignment {
                message_id: MessageId::new(),
                task_id: task_id.clone(),
                command: Command::Shell(uhorse_protocol::ShellCommand::new("sh")),
                priority: uhorse_protocol::Priority::Normal,
                deadline: None,
                context: TaskContext::new(
                    uhorse_protocol::UserId::from_string("test-user"),
                    uhorse_protocol::SessionId::new(),
                    "test-channel",
                ),
                retry_count: 0,
                max_retries: 3,
            },
            &executor,
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let approval_request = tokio::time::timeout(Duration::from_secs(1), outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();

        let request_id = match approval_request {
            NodeToHub::ApprovalRequest { request_id, .. } => request_id,
            other => panic!("unexpected message: {:?}", other),
        };

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::ApprovalResponse {
                message_id: MessageId::new(),
                request_id,
                approved: false,
                responder: "admin".to_string(),
                reason: Some("rejected by admin".to_string()),
                responded_at: Utc::now(),
            },
            &executor,
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let message = tokio::time::timeout(Duration::from_secs(1), outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match message {
            NodeToHub::TaskResult {
                task_id: result_task_id,
                result,
                ..
            } => {
                assert_eq!(result_task_id, task_id);
                assert!(!result.success);
                assert_eq!(
                    result.error.as_ref().map(|error| error.message.as_str()),
                    Some("rejected by admin")
                );
            }
            other => panic!("unexpected message: {:?}", other),
        }

        assert!(permission_manager.get_pending_approvals().await.is_empty());
        assert!(running_tasks.read().await.is_empty());
        assert_eq!(metrics.read().await.total_executions, 1);
        assert_eq!(metrics.read().await.failed_executions, 1);
    }

    #[tokio::test]
    async fn test_report_notification_event_nowait_enqueues_message() {
        let temp = TempDir::new().unwrap();
        let mut node = Node::new(NodeConfig {
            workspace_path: temp.path().to_string_lossy().to_string(),
            require_git_repo: false,
            ..Default::default()
        })
        .unwrap();
        let (outbound_tx, mut outbound_rx) = mpsc::channel(1);
        node.running.store(true, Ordering::SeqCst);
        node.outbound_tx = Some(outbound_tx);

        node.report_notification_event_nowait(NotificationEventKind::Info, "标题", "内容", true)
            .unwrap();

        let outbound = outbound_rx.recv().await.unwrap();
        match outbound {
            NodeToHub::NotificationEvent { node_id, event, .. } => {
                assert_eq!(node_id, *node.node_id());
                assert_eq!(event.title, "标题");
                assert_eq!(event.body, "内容");
                assert!(event.details_included);
                assert!(matches!(event.kind, NotificationEventKind::Info));
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_collect_heartbeat_snapshot_uses_running_task_count() {
        let temp = TempDir::new().unwrap();
        let node = Node::new(NodeConfig {
            workspace_path: temp.path().to_string_lossy().to_string(),
            require_git_repo: false,
            max_concurrent_tasks: 7,
            ..Default::default()
        })
        .unwrap();

        let task_id = TaskId::from_string("task-running");
        let (cancel_tx, _cancel_rx) = mpsc::channel(1);
        node.running_tasks
            .write()
            .await
            .insert(task_id.clone(), RunningTask { cancel_tx });

        let snapshot = Node::collect_heartbeat_snapshot(
            &node.status_reporter,
            1,
            node.config.max_concurrent_tasks,
            Some(42),
        )
        .await
        .unwrap();

        assert_eq!(snapshot.status.current_tasks, 1);
        assert_eq!(snapshot.status.max_tasks, 7);
        assert_eq!(snapshot.status.network_latency_ms, Some(42));
        assert_eq!(snapshot.load.task_count, 1);
        assert_eq!(snapshot.load.latency_ms, Some(42));
    }

    #[tokio::test]
    async fn test_handle_hub_message_permission_update_replaces_rules() {
        let temp = TempDir::new().unwrap();
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone(), true));
        permission_manager.load_default_rules().await;
        let executor = Arc::new(CommandExecutor::new(
            workspace,
            permission_manager.clone(),
            ".uhorse".to_string(),
        ));
        let metrics = Arc::new(RwLock::new(Metrics::default()));
        let running_tasks = Arc::new(RwLock::new(HashMap::new()));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(1);

        Node::handle_hub_message(
            &NodeId::from_string("node-1"),
            &HubToNode::PermissionUpdate {
                message_id: MessageId::new(),
                rules: vec![uhorse_protocol::PermissionRule {
                    id: "approval-shell".to_string(),
                    name: "Require shell approval".to_string(),
                    resource: uhorse_protocol::ResourcePattern::Prefix {
                        prefix: temp.path().to_string_lossy().to_string(),
                    },
                    actions: vec![uhorse_protocol::Action::Execute],
                    conditions: vec![],
                    require_approval: true,
                    enabled: true,
                }],
            },
            &executor,
            &permission_manager,
            &metrics,
            &running_tasks,
            &outbound_tx,
        )
        .await
        .unwrap();

        let command = Command::Shell(
            uhorse_protocol::ShellCommand::new("sh")
                .with_args(vec!["-c".to_string(), "printf ok".to_string()])
                .with_cwd(temp.path().to_string_lossy().to_string()),
        );
        let result = permission_manager
            .check(
                &command,
                &TaskContext::new(
                    uhorse_protocol::UserId::from_string("test-user"),
                    uhorse_protocol::SessionId::new(),
                    "test-channel",
                ),
            )
            .await;
        assert!(matches!(result, PermissionResult::RequiresApproval { .. }));

        let outbound = tokio::time::timeout(Duration::from_millis(200), outbound_rx.recv()).await;
        assert!(outbound.is_err());
    }
}
