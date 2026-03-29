//! 核心类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 节点 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    /// 生成新的节点 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// 从字符串创建
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 任务 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    /// 生成新的任务 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// 从字符串创建
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 用户 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    /// 从字符串创建
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 会话 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    /// 生成新的会话 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// 从字符串创建
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 技能 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId(pub String);

impl SkillId {
    /// 生成新的技能 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// 从字符串创建
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 技能版本
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillVersion {
    /// 主版本号
    pub major: u32,
    /// 次版本号
    pub minor: u32,
    /// 修订版本号
    pub patch: u32,
}

impl SkillVersion {
    /// 创建新的版本号
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// 解析版本字符串 (如 "1.2.3")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl Default for SkillVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

impl std::fmt::Display for SkillVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// 后台任务（最低优先级）
    Background,
    /// 低优先级
    Low,
    /// 普通优先级
    #[default]
    Normal,
    /// 高优先级
    High,
    /// 紧急
    Urgent,
    /// 关键任务（最高优先级）
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Background => write!(f, "background"),
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Urgent => write!(f, "urgent"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 已创建
    Created,
    /// 排队中
    Queued,
    /// 已分配
    Assigned,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
    /// 已超时
    Timeout,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Queued => write!(f, "queued"),
            Self::Assigned => write!(f, "assigned"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Timeout => write!(f, "timeout"),
        }
    }
}

/// 节点能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// 支持的命令类型
    pub supported_commands: Vec<CommandType>,

    /// 标签（用于任务路由）
    pub tags: Vec<String>,

    /// 最大并发任务数
    pub max_concurrent_tasks: usize,

    /// 可用工具
    pub available_tools: Vec<String>,
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self {
            supported_commands: vec![CommandType::File, CommandType::Shell, CommandType::Code],
            tags: vec!["default".to_string()],
            max_concurrent_tasks: 5,
            available_tools: vec![],
        }
    }
}

/// 命令类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    /// 文件操作
    File,
    /// Shell 命令
    Shell,
    /// 代码执行
    Code,
    /// 数据库查询
    Database,
    /// API 调用
    Api,
    /// 浏览器操作
    Browser,
    /// 自定义技能
    Skill,
}

/// 工作空间信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// 稳定执行工作空间标识
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,

    /// 工作空间名称
    pub name: String,

    /// 工作空间路径
    pub path: String,

    /// 是否只读
    pub read_only: bool,

    /// 允许的文件模式
    pub allowed_patterns: Vec<String>,

    /// 禁止的文件模式
    pub denied_patterns: Vec<String>,
}

/// 节点状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// 节点 ID
    pub node_id: NodeId,

    /// 是否在线
    pub online: bool,

    /// 当前任务数
    pub current_tasks: usize,

    /// 最大任务数
    pub max_tasks: usize,

    /// CPU 使用率
    pub cpu_percent: f32,

    /// 内存使用 (MB)
    pub memory_mb: u64,

    /// 磁盘使用 (GB)
    pub disk_gb: f64,

    /// 网络延迟 (ms)
    pub network_latency_ms: Option<u64>,

    /// 最后心跳时间
    pub last_heartbeat: DateTime<Utc>,
}

/// 负载信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadInfo {
    /// CPU 使用率 (0.0 - 1.0)
    pub cpu_usage: f32,

    /// 内存使用率 (0.0 - 1.0)
    pub memory_usage: f32,

    /// 当前任务数
    pub task_count: usize,

    /// 网络延迟 (ms)
    pub latency_ms: Option<u64>,
}

impl LoadInfo {
    /// 计算综合负载评分 (0.0 - 1.0)
    pub fn score(&self) -> f32 {
        let cpu_score = self.cpu_usage * 0.4;
        let mem_score = self.memory_usage * 0.3;
        let task_score = (self.task_count as f32 / 10.0).min(1.0) * 0.2;
        let latency_score = self
            .latency_ms
            .map(|l| (l as f32 / 1000.0).min(0.1))
            .unwrap_or(0.0);

        (cpu_score + mem_score + task_score + latency_score).min(1.0)
    }

    /// 计算综合负载（score 的别名）
    pub fn combined_load(&self) -> f32 {
        self.score()
    }
}

impl Default for LoadInfo {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            task_count: 0,
            latency_ms: None,
        }
    }
}

impl NodeCapabilities {
    /// 检查是否满足要求的能力
    pub fn meets(&self, required: &NodeCapabilities) -> bool {
        // 检查最大并发任务数
        if self.max_concurrent_tasks < required.max_concurrent_tasks {
            return false;
        }

        // 检查支持的命令类型
        for cmd in &required.supported_commands {
            if !self.supported_commands.contains(cmd) {
                return false;
            }
        }

        // 检查需要的工具
        for tool in &required.available_tools {
            if !self.available_tools.contains(tool) {
                return false;
            }
        }

        true
    }
}

/// 任务上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    /// 用户 ID
    pub user_id: UserId,

    /// 会话 ID
    pub session_id: SessionId,

    /// 消息来源渠道
    pub channel: String,

    /// 目标执行工作空间标识
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_workspace_id: Option<String>,

    /// 逻辑协作工作空间标识
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration_workspace_id: Option<String>,

    /// 用户意图描述
    pub intent: Option<String>,

    /// 环境变量
    pub env: HashMap<String, String>,

    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl TaskContext {
    /// 创建新的任务上下文
    pub fn new(user_id: UserId, session_id: SessionId, channel: impl Into<String>) -> Self {
        Self {
            user_id,
            session_id,
            channel: channel.into(),
            execution_workspace_id: None,
            collaboration_workspace_id: None,
            intent: None,
            env: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// 设置执行工作空间标识
    pub fn with_execution_workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.execution_workspace_id = Some(workspace_id.into());
        self
    }

    /// 设置协作工作空间标识
    pub fn with_collaboration_workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.collaboration_workspace_id = Some(workspace_id.into());
        self
    }

    /// 设置意图
    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    /// 添加环境变量
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// 执行指标
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionMetrics {
    /// 执行时间 (ms)
    pub duration_ms: u64,

    /// CPU 时间 (ms)
    pub cpu_time_ms: u64,

    /// 内存峰值 (MB)
    pub peak_memory_mb: u64,

    /// 读取字节数
    pub bytes_read: u64,

    /// 写入字节数
    pub bytes_written: u64,

    /// 网络请求数
    pub network_requests: u64,
}
