# uHorse 4.0 架构设计

## 版本愿景

**uHorse 4.0** 定位为"分布式 AI 工作编排平台"，实现云端中枢 + 本地节点的分布式架构：

```
3.0: 企业级 AI 基础设施平台 → 4.0: 分布式 AI 工作编排平台
```

### 核心理念

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              用户交互层                                      │
│         钉钉 | Slack | Discord | WhatsApp | Telegram | Web | CLI            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓ 消息
┌─────────────────────────────────────────────────────────────────────────────┐
│                         云端中枢 (Hub)                                       │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │ 模型    │ │ 记忆    │ │ 技能    │ │ Agent   │ │ 任务    │ │ 通道    │   │
│  │ 管理    │ │ 管理    │ │ 管理    │ │ 编排    │ │ 调度    │ │ 对接    │   │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘   │
│                                                                              │
│  理解意图 → 检索记忆 → 选择技能 → 规划任务 → 分发命令 → 汇总结果 → 返回用户  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓ 命令/任务
┌─────────────────────────────────────────────────────────────────────────────┐
│                          本地节点 (Node)                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  工作目录: /Users/xxx/projects/                                     │    │
│  │  权限: 文件读写、代码执行、数据库访问、API调用                       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  接收任务 → 执行操作 → 返回结果                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 与现有架构对比

| 维度 | 3.x 现状 | 4.0 目标 |
|------|----------|----------|
| **架构模式** | 单体/集群 | 中枢-节点分布式 |
| **执行位置** | 服务端执行 | 中枢规划 + 节点执行 |
| **文件访问** | 服务端文件系统 | 用户本地文件系统 |
| **权限模型** | 服务端权限 | 用户授权 + 节点权限 |
| **任务分发** | 无 | 支持多节点并行 |
| **离线能力** | 需联网 | 节点可离线执行 |

---

## 架构设计

### 1. 云端中枢 (Hub)

#### 1.1 核心职责

```rust
/// 云端中枢 - 统一智能编排中心
pub struct Hub {
    /// 模型管理器 - 多 LLM 路由与负载均衡
    model_manager: ModelManager,

    /// 记忆管理器 - 向量存储 + 上下文管理
    memory_manager: MemoryManager,

    /// 技能管理器 - 技能注册、发现、版本管理
    skill_manager: SkillManager,

    /// Agent 编排器 - 多 Agent 协作与任务分解
    agent_orchestrator: AgentOrchestrator,

    /// 任务调度器 - 任务队列、优先级、依赖管理
    task_scheduler: TaskScheduler,

    /// 通道管理器 - 多渠道消息接入
    channel_manager: ChannelManager,

    /// 节点管理器 - 节点注册、心跳、负载监控
    node_manager: NodeManager,
}
```

#### 1.2 模块结构

```
crates/uhorse-hub/
├── Cargo.toml
└── src/
    ├── lib.rs                    # Hub 入口
    ├── model/                    # 模型管理
    │   ├── mod.rs
    │   ├── manager.rs            # 多模型路由
    │   ├── load_balancer.rs      # 负载均衡
    │   └── cost_tracker.rs       # 成本追踪
    ├── memory/                   # 记忆管理
    │   ├── mod.rs
    │   ├── manager.rs            # 记忆管理器
    │   ├── vector_store.rs       # 向量存储接口
    │   ├── context.rs            # 上下文管理
    │   └── retrieval.rs          # 检索增强
    ├── skill/                    # 技能管理
    │   ├── mod.rs
    │   ├── manager.rs            # 技能管理器
    │   ├── registry.rs           # 技能注册表
    │   └── versioning.rs         # 版本管理
    ├── agent/                    # Agent 编排
    │   ├── mod.rs
    │   ├── orchestrator.rs       # 编排器
    │   ├── planner.rs            # 任务规划
    │   ├── executor.rs           # 执行协调
    │   └── collaboration.rs      # 多 Agent 协作
    ├── task/                     # 任务调度
    │   ├── mod.rs
    │   ├── scheduler.rs          # 调度器
    │   ├── queue.rs              # 任务队列
    │   ├── priority.rs           # 优先级管理
    │   └── dependency.rs         # 依赖管理
    ├── channel/                  # 通道管理
    │   ├── mod.rs
    │   ├── manager.rs            # 通道管理器
    │   └── adapters/             # 各通道适配器
    ├── node/                     # 节点管理
    │   ├── mod.rs
    │   ├── manager.rs            # 节点管理器
    │   ├── registry.rs           # 节点注册
    │   ├── heartbeat.rs          # 心跳检测
    │   └── load_monitor.rs       # 负载监控
    ├── command/                  # 命令系统
    │   ├── mod.rs
    │   ├── types.rs              # 命令类型定义
    │   ├── builder.rs            # 命令构建器
    │   └── validator.rs          # 命令验证
    └── session/                  # 会话管理
        ├── mod.rs
        ├── manager.rs            # 会话管理器
        └── context.rs            # 会话上下文
```

#### 1.3 工作流程

```rust
/// Hub 处理用户消息的完整流程
impl Hub {
    pub async fn process_message(&self, message: IncomingMessage) -> Result<OutgoingMessage> {
        // 1. 解析消息来源和上下文
        let context = self.parse_context(&message).await?;

        // 2. 使用 LLM 理解用户意图
        let intent = self.model_manager.understand_intent(&message, &context).await?;

        // 3. 检索相关记忆
        let memories = self.memory_manager.retrieve(&intent, &context).await?;

        // 4. 选择合适的技能
        let skills = self.skill_manager.select_skills(&intent, &memories).await?;

        // 5. 规划任务
        let plan = self.agent_orchestrator.plan(&intent, &skills, &memories).await?;

        // 6. 选择目标节点
        let nodes = self.node_manager.select_nodes(&plan).await?;

        // 7. 分发任务给节点
        let results = self.dispatch_tasks(&plan, &nodes).await?;

        // 8. 汇总结果
        let summary = self.agent_orchestrator.summarize(&results).await?;

        // 9. 生成回复
        let response = self.model_manager.generate_response(&summary, &context).await?;

        // 10. 保存记忆
        self.memory_manager.store(&message, &response, &context).await?;

        Ok(response)
    }
}
```

---

### 2. 本地节点 (Node)

#### 2.1 核心职责

```rust
/// 本地节点 - 执行中枢下发的命令
pub struct Node {
    /// 节点 ID
    id: NodeId,

    /// 工作目录 - 用户指定的操作范围
    workspace: Workspace,

    /// 权限管理 - 操作授权
    permissions: PermissionManager,

    /// 命令执行器 - 执行各类命令
    executor: CommandExecutor,

    /// Hub 连接 - 与中枢通信
    hub_connection: HubConnection,

    /// 本地工具 - 文件操作、代码执行等
    local_tools: LocalToolRegistry,

    /// 状态报告 - 定期上报状态
    status_reporter: StatusReporter,
}
```

#### 2.2 模块结构

```
crates/uhorse-node/
├── Cargo.toml
└── src/
    ├── lib.rs                    # Node 入口
    ├── workspace/                # 工作空间管理
    │   ├── mod.rs
    │   ├── manager.rs            # 工作空间管理器
    │   ├── watcher.rs            # 文件变更监听
    │   └── index.rs              # 文件索引
    ├── permission/               # 权限管理
    │   ├── mod.rs
    │   ├── manager.rs            # 权限管理器
    │   ├── rules.rs              # 权限规则
    │   └── audit.rs              # 操作审计
    ├── executor/                 # 命令执行
    │   ├── mod.rs
    │   ├── executor.rs           # 执行器
    │   ├── sandbox.rs            # 沙箱环境
    │   └── process.rs            # 进程管理
    ├── connection/               # Hub 连接
    │   ├── mod.rs
    │   ├── websocket.rs          # WebSocket 连接
    │   ├── reconnect.rs          # 重连机制
    │   └── auth.rs               # 节点认证
    ├── tools/                    # 本地工具
    │   ├── mod.rs
    │   ├── file.rs               # 文件操作
    │   ├── code.rs               # 代码执行
    │   ├── database.rs           # 数据库访问
    │   ├── api.rs                # API 调用
    │   ├── shell.rs              # Shell 命令
    │   └── browser.rs            # 浏览器操作
    ├── status/                   # 状态报告
    │   ├── mod.rs
    │   ├── reporter.rs           # 状态上报
    │   └── metrics.rs            # 指标收集
    └── cli/                      # 命令行接口
        ├── mod.rs
        ├── main.rs               # 入口
        └── commands.rs           # CLI 命令
```

#### 2.3 命令类型

```rust
/// 节点可执行的命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// 文件操作
    File(FileCommand),

    /// 代码执行
    Code(CodeCommand),

    /// 数据库查询
    Database(DatabaseCommand),

    /// API 调用
    Api(ApiCommand),

    /// Shell 命令
    Shell(ShellCommand),

    /// 浏览器操作
    Browser(BrowserCommand),

    /// 自定义技能执行
    Skill(SkillCommand),
}

/// 文件操作命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileCommand {
    /// 读取文件
    Read { path: String, limit: Option<usize> },

    /// 写入文件
    Write { path: String, content: String },

    /// 追加内容
    Append { path: String, content: String },

    /// 删除文件/目录
    Delete { path: String, recursive: bool },

    /// 列出目录
    List { path: String, recursive: bool },

    /// 搜索文件
    Search { pattern: String, path: String },

    /// 复制文件
    Copy { from: String, to: String },

    /// 移动文件
    Move { from: String, to: String },

    /// 获取文件信息
    Info { path: String },
}

/// 代码执行命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeCommand {
    /// 语言
    pub language: CodeLanguage,

    /// 代码内容
    pub code: String,

    /// 超时时间
    pub timeout: Duration,

    /// 环境变量
    pub env: HashMap<String, String>,
}

/// 数据库查询命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseCommand {
    /// 数据库类型
    pub db_type: DatabaseType,

    /// 连接字符串（或引用预配置的连接）
    pub connection: String,

    /// SQL 查询
    pub query: String,

    /// 参数
    pub params: Vec<Value>,
}
```

#### 2.4 权限模型

```rust
/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// 规则 ID
    pub id: String,

    /// 资源路径模式
    pub resource: ResourcePattern,

    /// 允许的操作
    pub actions: Vec<Action>,

    /// 条件（如时间段、来源等）
    pub conditions: Vec<Condition>,

    /// 是否需要审批
    pub require_approval: bool,
}

/// 资源模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourcePattern {
    /// 精确路径
    Exact(String),

    /// 通配符模式
    Glob(String),

    /// 正则表达式
    Regex(String),

    /// 前缀匹配
    Prefix(String),
}

/// 操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// 读取
    Read,

    /// 写入
    Write,

    /// 删除
    Delete,

    /// 执行
    Execute,

    /// 列出
    List,

    /// 管理（修改权限）
    Admin,
}

/// 权限检查器
impl PermissionManager {
    pub fn check(&self, command: &Command, context: &CommandContext) -> PermissionResult {
        // 1. 获取命令需要的权限
        let required = command.required_permissions();

        // 2. 检查工作空间限制
        if !self.workspace.allows(&command) {
            return PermissionResult::Denied("Outside workspace".into());
        }

        // 3. 检查权限规则
        for rule in &self.rules {
            if rule.matches(&command, context) {
                if rule.require_approval {
                    return PermissionResult::RequiresApproval(rule.id.clone());
                }
                return PermissionResult::Allowed;
            }
        }

        // 4. 默认拒绝
        PermissionResult::Denied("No matching rule".into())
    }
}
```

---

### 3. 通信协议

#### 3.1 Hub-Node 通信

```rust
/// Hub 到 Node 的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubToNode {
    /// 任务分配
    TaskAssignment {
        task_id: TaskId,
        command: Command,
        priority: Priority,
        deadline: Option<DateTime<Utc>>,
        context: TaskContext,
    },

    /// 任务取消
    TaskCancellation {
        task_id: TaskId,
        reason: String,
    },

    /// 心跳请求
    HeartbeatRequest {
        timestamp: DateTime<Utc>,
    },

    /// 配置更新
    ConfigUpdate {
        config: NodeConfig,
    },

    /// 权限更新
    PermissionUpdate {
        rules: Vec<PermissionRule>,
    },
}

/// Node 到 Hub 的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeToHub {
    /// 注册请求
    Register {
        node_id: NodeId,
        capabilities: NodeCapabilities,
        workspace: WorkspaceInfo,
    },

    /// 心跳响应
    Heartbeat {
        node_id: NodeId,
        status: NodeStatus,
        load: LoadInfo,
        timestamp: DateTime<Utc>,
    },

    /// 任务结果
    TaskResult {
        task_id: TaskId,
        result: CommandResult,
        metrics: ExecutionMetrics,
    },

    /// 任务进度
    TaskProgress {
        task_id: TaskId,
        progress: f32,
        message: String,
    },

    /// 错误报告
    Error {
        task_id: Option<TaskId>,
        error: NodeError,
    },

    /// 权限审批请求
    ApprovalRequest {
        request_id: String,
        command: Command,
        context: CommandContext,
    },
}

/// 命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// 是否成功
    pub success: bool,

    /// 输出内容
    pub output: CommandOutput,

    /// 执行时间
    pub duration: Duration,

    /// 资源使用
    pub resources: ResourceUsage,

    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 命令输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandOutput {
    /// 文本输出
    Text(String),

    /// JSON 输出
    Json(Value),

    /// 二进制输出（引用）
    Binary {
        mime_type: String,
        size: usize,
        storage_ref: String,
    },

    /// 文件列表
    FileList(Vec<FileInfo>),

    /// 空（无输出）
    None,
}
```

#### 3.2 消息格式示例

```json
// Hub -> Node: 任务分配
{
  "type": "TaskAssignment",
  "task_id": "task-2024-001",
  "command": {
    "File": {
      "Search": {
        "pattern": "*.sql",
        "path": "/workspace/queries"
      }
    }
  },
  "priority": "Normal",
  "deadline": "2024-01-15T10:30:00Z",
  "context": {
    "user_id": "user-123",
    "session_id": "sess-456",
    "intent": "查找客户订单相关查询"
  }
}

// Node -> Hub: 任务结果
{
  "type": "TaskResult",
  "task_id": "task-2024-001",
  "result": {
    "success": true,
    "output": {
      "FileList": [
        {"path": "/workspace/queries/customer_orders.sql", "size": 1024},
        {"path": "/workspace/queries/order_summary.sql", "size": 512}
      ]
    },
    "duration_ms": 45,
    "resources": {
      "cpu_percent": 2.5,
      "memory_mb": 12
    }
  }
}
```

---

### 4. 任务调度

#### 4.1 任务生命周期

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│ Created │────>│ Queued  │────>│ Assigned│────>│ Running │────>│Complete │
└─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
                    │               │               │
                    │               │               │
                    ↓               ↓               ↓
               ┌─────────┐    ┌─────────┐    ┌─────────┐
               │ Expired │    │ Timeout │    │ Failed  │
               └─────────┘    └─────────┘    └─────────┘
```

#### 4.2 调度策略

```rust
/// 任务调度器
pub struct TaskScheduler {
    /// 任务队列（按优先级）
    queues: HashMap<Priority, TaskQueue>,

    /// 任务依赖图
    dependency_graph: DependencyGraph,

    /// 节点选择策略
    node_selector: NodeSelector,

    /// 超时管理
    timeout_manager: TimeoutManager,
}

impl TaskScheduler {
    /// 调度任务
    pub async fn schedule(&self, task: Task) -> ScheduleResult {
        // 1. 检查依赖
        if !self.dependency_graph.ready(&task) {
            return ScheduleResult::WaitingForDependencies;
        }

        // 2. 选择目标节点
        let node = self.node_selector.select(&task).await?;

        // 3. 检查节点负载
        if node.load > node.capacity * 0.9 {
            return ScheduleResult::NodeOverloaded;
        }

        // 4. 分配任务
        node.assign_task(task.clone()).await?;

        // 5. 设置超时
        self.timeout_manager.set_timeout(&task.id, task.timeout);

        ScheduleResult::Assigned { node_id: node.id }
    }

    /// 处理任务结果
    pub async fn handle_result(&self, result: TaskResult) {
        // 1. 取消超时
        self.timeout_manager.cancel(&result.task_id);

        // 2. 更新依赖图
        self.dependency_graph.complete(&result.task_id);

        // 3. 触发下游任务
        let ready = self.dependency_graph.get_ready_tasks();
        for task in ready {
            self.schedule(task).await;
        }
    }
}
```

---

### 5. 安全模型

#### 5.1 认证与授权

```
┌─────────────────────────────────────────────────────────────────┐
│                        认证层                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ Hub 认证    │  │ Node 认证   │  │ 用户认证    │             │
│  │ (JWT/MTLS)  │  │ (证书/Token)│  │ (OAuth2)    │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        授权层                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ 命令授权    │  │ 资源授权    │  │ 操作授权    │             │
│  │ (RBAC)      │  │ (ABAC)      │  │ (审批流程)  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        审计层                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ 命令审计    │  │ 访问审计    │  │ 变更审计    │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

#### 5.2 审批流程

```rust
/// 敏感操作审批
pub struct ApprovalFlow {
    /// 审批策略
    policy: ApprovalPolicy,

    /// 待审批请求
    pending: HashMap<String, ApprovalRequest>,

    /// 通知渠道
    notifier: NotificationChannel,
}

impl ApprovalFlow {
    /// 请求审批
    pub async fn request(&self, command: &Command, context: &CommandContext) -> ApprovalResult {
        // 1. 创建审批请求
        let request = ApprovalRequest::new(command, context);

        // 2. 发送通知给审批人
        self.notifier.notify(&request).await?;

        // 3. 等待审批（或超时）
        match self.wait_for_approval(&request.id).await {
            Some(true) => ApprovalResult::Approved,
            Some(false) => ApprovalResult::Rejected,
            None => ApprovalResult::Timeout,
        }
    }

    /// 处理审批响应
    pub async fn handle_response(&self, response: ApprovalResponse) {
        if let Some(request) = self.pending.get(&response.request_id) {
            request.resolve(response.approved);
        }
    }
}
```

---

### 6. 端到端流程示例

#### 示例：用户通过钉钉查询客户订单

```
1. 用户发送钉钉消息
   "帮我找到xxx客户的最近半年的订单数据"
   ↓

2. 消息到达 Hub (通过钉钉 Channel Adapter)
   ↓

3. Hub 处理流程:
   a. 解析消息 → 用户: 张三, 渠道: 钉钉, 意图: 数据查询
   b. LLM 理解意图 → 需要查询订单数据库
   c. 检索记忆 → 用户之前配置了 CRM 数据库连接
   d. 选择技能 → database_query 技能
   e. 规划任务:
      - Task 1: 在节点 A 搜索 SQL 查询模板
      - Task 2: 连接 CRM 数据库执行查询
      - Task 3: 格式化结果
   f. 选择节点 → Node-Work-PC (有数据库访问权限)
   g. 分发任务 → 发送 DatabaseCommand 给节点
   ↓

4. Node 执行:
   a. 接收命令 → DatabaseCommand { query: "..." }
   b. 权限检查 → 用户张三有查询权限
   c. 执行查询 → 连接数据库, 执行 SQL
   d. 返回结果 → 1234 条订单记录
   ↓

5. Hub 汇总:
   a. 接收结果 → 1234 条记录
   b. LLM 生成摘要 → "找到 xxx 客户最近半年订单 1234 条..."
   c. 格式化为钉钉消息
   d. 保存到记忆
   ↓

6. 返回用户 (钉钉):
   "找到 xxx 客户最近半年订单 1234 条：
    - 总金额: ¥1,234,567
    - 最近订单: 2024-01-10 #12345
    [查看详情] [导出Excel]"
```

---

## 模块迁移计划

### 保留模块 (3.x → 4.0)

| 3.x 模块 | 4.0 位置 | 说明 |
|----------|----------|------|
| uhorse-core | 共用 | 核心类型定义 |
| uhorse-llm | uhorse-hub/model | 移至 Hub |
| uhorse-agent | uhorse-hub/agent | 移至 Hub |
| uhorse-channel | uhorse-hub/channel | 移至 Hub |
| uhorse-session | uhorse-hub/session | 移至 Hub |
| uhorse-storage | 共用 | 数据存储 |
| uhorse-discovery | uhorse-hub/node | 用于节点发现 |
| uhorse-cache | 共用 | 缓存层 |
| uhorse-queue | 共用 | 任务队列 |
| uhorse-security | 共用 | 安全模块 |
| uhorse-observability | 共用 | 可观测性 |
| uhorse-config | 共用 | 配置管理 |

### 新增模块

| 模块 | 说明 |
|------|------|
| uhorse-hub | 云端中枢（整合现有模块） |
| uhorse-node | 本地节点 |
| uhorse-protocol | Hub-Node 通信协议 |
| uhorse-workspace | 工作空间管理 |
| uhorse-permission | 权限管理 |

---

## 实施路线图

### Phase 1: 核心架构 ✅ 已完成 (4 周)

```
Week 1-2: 协议设计 ✅
├── uhorse-protocol crate
├── Hub-Node 通信协议
├── 命令类型定义
└── 消息序列化

Week 3-4: 节点基础 ✅
├── uhorse-node crate 骨架
├── 工作空间管理
├── 命令执行器
└── Hub 连接
```

**已完成功能**:
- [x] uhorse-protocol crate - 完整的 Hub-Node 通信协议
- [x] Command 枚举类型 (File/Shell/Code/Database/Api/Browser/Skill)
- [x] NodeManager - 节点注册、心跳检测、负载监控
- [x] TaskScheduler - 任务调度、优先级队列、超时控制
- [x] MessageRouter - 消息路由、命令分发

### Phase 2: 中枢构建 ✅ 已完成 (4 周)

```
Week 5-6: Hub 集成 ✅
├── uhorse-hub crate
├── 整合 LLM/Agent/Channel
├── 任务调度器
└── 节点管理器

Week 7-8: 智能编排 ✅
├── 意图理解
├── 任务规划
├── 节点选择
└── 结果汇总
```

**已完成功能**:
- [x] Orchestrator - 智能编排器 (crates/uhorse-hub/src/orchestrator.rs)
- [x] OrchestrationPlan - 编排计划 (意图、子任务、依赖关系)
- [x] 复用 uhorse-agent::SkillRegistry - 技能注册表
- [x] 子任务依赖管理 - 拓扑排序并行执行
- [x] 结果汇总 - SubTaskResult 聚合与状态跟踪

### Phase 3: 安全与权限 ✅ 已完成 (3 周)

```
Week 9-10: 权限系统 ✅
├── uhorse-permission crate
├── 权限规则引擎
├── 审批流程
└── 审计日志

Week 11: 安全加固 ✅
├── 节点认证
├── 通信加密
└── 敏感操作保护
```

**已完成功能**:
- [x] NodeAuthenticator - JWT 节点认证 (复用 uhorse-security::JwtAuthService)
- [x] SensitiveOperationApprover - 敏感操作审批器 (复用 ApprovalManager)
- [x] HubFieldEncryptor - 字段级加密 (复用 FieldEncryptor)
- [x] HubTlsConfig - TLS 配置包装
- [x] SecurityManager - 安全组件整合
- [x] IdempotencyCache - 幂等性控制

### Phase 4: 工具与集成 🚧 规划中 (3 周)

```
Week 12-13: 本地工具
├── 文件操作工具
├── 代码执行工具
├── 数据库工具
└── API 调用工具

Week 14: 通道集成
├── 钉钉集成测试
├── Slack 集成测试
└── Web 界面
```

### Phase 5: 测试与优化 📅 待开始 (2 周)

```
Week 15-16: 验证
├── 端到端测试
├── 性能测试
├── 安全测试
└── v4.0.0 发布
```

---

## 关键技术决策

### 1. 通信协议

**选择**: WebSocket + 自定义二进制协议

**理由**:
- WebSocket 支持双向实时通信
- 自定义协议可优化传输效率
- 支持消息确认和重传

### 2. 节点发现

**选择**: Hub 集中式注册 + 心跳

**理由**:
- 简化部署，无需额外服务发现组件
- Hub 可全局调度
- 支持节点动态上下线

### 3. 任务队列

**选择**: Redis + 优先级队列

**理由**:
- 复用 3.x 的 Redis 基础设施
- 支持持久化和分布式
- 优先级保证重要任务优先执行

### 4. 权限模型

**选择**: RBAC + ABAC 混合

**理由**:
- RBAC 适合用户角色管理
- ABAC 适合资源级别控制
- 混合模式灵活性最高

---

## 总结

**uHorse 4.0** 将实现从"企业级 AI 基础设施平台"到"分布式 AI 工作编排平台"的升级：

### 当前完成进度

| Phase | 名称 | 状态 | 完成内容 |
|-------|------|------|----------|
| **Phase 1** | 核心架构 | ✅ 完成 | uhorse-protocol, uhorse-hub, uhorse-node 骨架 |
| **Phase 2** | 智能编排 | ✅ 完成 | Orchestrator, 任务规划, 结果汇总 |
| **Phase 3** | 安全加固 | ✅ 完成 | JWT认证, 敏感操作审批, 字段加密, TLS |
| **Phase 4** | 工具集成 | ✅ 完成 | 本地工具, 通道集成, Web 管理界面 |
| **Phase 5** | 测试优化 | 📅 待开始 | 端到端测试, 性能测试 |

### 已实现核心功能

✅ **云端中枢 (Hub)**:
- NodeManager - 节点注册、心跳检测、负载监控
- TaskScheduler - 任务调度、优先级队列、超时控制、重试机制
- MessageRouter - 消息路由、命令分发、结果汇总
- Orchestrator - 意图理解、任务规划、结果汇总

✅ **通信协议 (Protocol)**:
- HubToNode/NodeToHub 消息类型
- Command 命令类型 (File/Shell/Code/Database/Api/Browser/Skill)
- Priority 优先级 (Critical/Urgent/High/Normal/Low/Background)
- NodeCapabilities 节点能力声明

✅ **安全模型 (Security)**:
- NodeAuthenticator - JWT 节点认证
- SensitiveOperationApprover - 敏感操作审批
- HubFieldEncryptor - 字段级加密
- HubTlsConfig - TLS 配置
- IdempotencyCache - 幂等性控制

### 架构原则

- **复用优先**: 最大程度复用 3.x 已有模块能力
- **最小改动**: 在现有模块基础上扩展，而非重写
- **渐进升级**: 保持向后兼容，平滑过渡

**版本**: v4.0.0-alpha (Phase 1-3 完成)
**下一步**: Phase 4 工具与集成
