//! 消息定义
//!
//! 定义 Hub 和 Node 之间的通信消息格式

use crate::{command::*, result::*, types::*};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 消息 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

impl MessageId {
    /// 生成新的消息 ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Hub -> Node 消息
// ============================================================================

/// Hub 发送给 Node 的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "direction", rename_all = "snake_case")]
pub enum HubToNode {
    /// 任务分配
    TaskAssignment {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 任务 ID
        task_id: TaskId,

        /// 要执行的命令
        command: Command,

        /// 优先级
        #[serde(default)]
        priority: Priority,

        /// 截止时间
        deadline: Option<DateTime<Utc>>,

        /// 任务上下文
        context: TaskContext,

        /// 重试次数
        #[serde(default)]
        retry_count: u32,

        /// 最大重试次数
        #[serde(default = "default_max_retries")]
        max_retries: u32,
    },

    /// 任务取消
    TaskCancellation {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 任务 ID
        task_id: TaskId,

        /// 取消原因
        reason: String,
    },

    /// 心跳请求
    HeartbeatRequest {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 配置更新
    ConfigUpdate {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 配置内容（JSON）
        config: serde_json::Value,
    },

    /// 权限更新
    PermissionUpdate {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 权限规则
        rules: Vec<PermissionRule>,
    },

    /// 技能部署
    SkillDeploy {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 技能名称
        skill_name: String,

        /// 技能版本
        version: String,

        /// 技能定义（SKILL.md 内容）
        skill_definition: String,

        /// 执行代码（WASM 或脚本）
        code: Option<Vec<u8>>,
    },

    /// 技能移除
    SkillRemove {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 技能名称
        skill_name: String,
    },

    /// 审批响应
    ApprovalResponse {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 审批请求 ID
        request_id: String,

        /// 是否批准
        approved: bool,

        /// 响应人
        responder: String,

        /// 响应备注
        reason: Option<String>,

        /// 响应时间
        responded_at: DateTime<Utc>,
    },

    /// 断开连接通知
    Disconnect {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 原因
        reason: String,

        /// 是否可重连
        reconnectable: bool,
    },
}

impl HubToNode {
    /// 获取消息 ID
    pub fn message_id(&self) -> &MessageId {
        match self {
            Self::TaskAssignment { message_id, .. } => message_id,
            Self::TaskCancellation { message_id, .. } => message_id,
            Self::HeartbeatRequest { message_id, .. } => message_id,
            Self::ConfigUpdate { message_id, .. } => message_id,
            Self::PermissionUpdate { message_id, .. } => message_id,
            Self::SkillDeploy { message_id, .. } => message_id,
            Self::SkillRemove { message_id, .. } => message_id,
            Self::ApprovalResponse { message_id, .. } => message_id,
            Self::Disconnect { message_id, .. } => message_id,
        }
    }

    /// 获取消息类型名称
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::TaskAssignment { .. } => "task_assignment",
            Self::TaskCancellation { .. } => "task_cancellation",
            Self::HeartbeatRequest { .. } => "heartbeat_request",
            Self::ConfigUpdate { .. } => "config_update",
            Self::PermissionUpdate { .. } => "permission_update",
            Self::SkillDeploy { .. } => "skill_deploy",
            Self::SkillRemove { .. } => "skill_remove",
            Self::ApprovalResponse { .. } => "approval_response",
            Self::Disconnect { .. } => "disconnect",
        }
    }
}

// ============================================================================
// Node -> Hub 消息
// ============================================================================

/// Node 发送给 Hub 的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "direction", rename_all = "snake_case")]
pub enum NodeToHub {
    /// 注册请求
    Register {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 节点 ID
        node_id: NodeId,

        /// 节点名称
        name: String,

        /// 节点能力
        capabilities: NodeCapabilities,

        /// 工作空间信息
        workspace: WorkspaceInfo,

        /// 认证令牌
        auth_token: String,

        /// 注册时间
        timestamp: DateTime<Utc>,
    },

    /// 心跳响应
    Heartbeat {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 节点 ID
        node_id: NodeId,

        /// 节点状态
        status: NodeStatus,

        /// 负载信息
        load: LoadInfo,

        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 任务结果
    TaskResult {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 任务 ID
        task_id: TaskId,

        /// 执行结果
        result: CommandResult,

        /// 执行指标
        metrics: ExecutionMetrics,
    },

    /// 任务进度
    TaskProgress {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 任务 ID
        task_id: TaskId,

        /// 进度 (0.0 - 1.0)
        progress: f32,

        /// 进度消息
        message: String,

        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 错误报告
    Error {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 相关任务 ID（如果有）
        task_id: Option<TaskId>,

        /// 错误信息
        error: NodeError,

        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 审批请求
    ApprovalRequest {
        /// 消息息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 请求 ID
        request_id: String,

        /// 任务 ID
        task_id: TaskId,

        /// 需要审批的命令
        command: Command,

        /// 命令上下文
        context: TaskContext,

        /// 审批原因
        reason: String,

        /// 请求时间
        timestamp: DateTime<Utc>,

        /// 过期时间
        expires_at: DateTime<Utc>,
    },

    /// 注销请求
    Unregister {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 节点 ID
        node_id: NodeId,

        /// 注销原因
        reason: String,
    },

    /// 技能列表报告
    SkillReport {
        /// 消息 ID
        #[serde(default = "MessageId::new")]
        message_id: MessageId,

        /// 节点 ID
        node_id: NodeId,

        /// 已安装的技能
        installed_skills: Vec<InstalledSkill>,
    },
}

impl NodeToHub {
    /// 获取消息 ID
    pub fn message_id(&self) -> &MessageId {
        match self {
            Self::Register { message_id, .. } => message_id,
            Self::Heartbeat { message_id, .. } => message_id,
            Self::TaskResult { message_id, .. } => message_id,
            Self::TaskProgress { message_id, .. } => message_id,
            Self::Error { message_id, .. } => message_id,
            Self::ApprovalRequest { message_id, .. } => message_id,
            Self::Unregister { message_id, .. } => message_id,
            Self::SkillReport { message_id, .. } => message_id,
        }
    }

    /// 获取消息类型名称
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::Register { .. } => "register",
            Self::Heartbeat { .. } => "heartbeat",
            Self::TaskResult { .. } => "task_result",
            Self::TaskProgress { .. } => "task_progress",
            Self::Error { .. } => "error",
            Self::ApprovalRequest { .. } => "approval_request",
            Self::Unregister { .. } => "unregister",
            Self::SkillReport { .. } => "skill_report",
        }
    }

    /// 获取节点 ID（如果消息中包含）
    pub fn node_id(&self) -> Option<&NodeId> {
        match self {
            Self::Register { node_id, .. } => Some(node_id),
            Self::Heartbeat { node_id, .. } => Some(node_id),
            Self::Unregister { node_id, .. } => Some(node_id),
            Self::SkillReport { node_id, .. } => Some(node_id),
            _ => None,
        }
    }
}

// ============================================================================
// 权限规则
// ============================================================================

/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// 规则 ID
    pub id: String,

    /// 规则名称
    pub name: String,

    /// 资源模式
    pub resource: ResourcePattern,

    /// 允许的操作
    pub actions: Vec<Action>,

    /// 条件
    pub conditions: Vec<Condition>,

    /// 是否需要审批
    #[serde(default)]
    pub require_approval: bool,

    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 资源模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourcePattern {
    /// 精确路径
    Exact {
        /// 路径
        path: String,
    },

    /// 通配符模式
    Glob {
        /// 模式
        pattern: String,
    },

    /// 正则表达式
    Regex {
        /// 表达式
        pattern: String,
    },

    /// 前缀匹配
    Prefix {
        /// 前缀
        prefix: String,
    },
}

/// 操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// 管理
    Admin,
}

/// 条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// 时间范围
    TimeRange {
        /// 开始时间 (HH:MM)
        start: String,
        /// 结束时间 (HH:MM)
        end: String,
    },

    /// IP 白名单
    IpWhitelist {
        /// IP 列表
        ips: Vec<String>,
    },

    /// 用户限制
    UserRestriction {
        /// 允许的用户 ID
        allowed_users: Vec<String>,
    },

    /// 大小限制
    SizeLimit {
        /// 最大字节数
        max_bytes: u64,
    },
}

// ============================================================================
// 节点错误
// ============================================================================

/// 节点错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeError {
    /// 错误代码
    pub code: String,

    /// 错误消息
    pub message: String,

    /// 错误详情
    pub details: Option<serde_json::Value>,

    /// 是否可重试
    #[serde(default)]
    pub retryable: bool,

    /// 堆栈跟踪（调试模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
}

impl NodeError {
    /// 创建新的节点错误
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            retryable: false,
            stack_trace: None,
        }
    }

    /// 设置详情
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// 设置是否可重试
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for NodeError {}

// ============================================================================
// 已安装技能
// ============================================================================

/// 已安装的技能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    /// 技能名称
    pub name: String,

    /// 版本
    pub version: String,

    /// 安装时间
    pub installed_at: DateTime<Utc>,

    /// 状态
    pub status: SkillStatus,
}

/// 技能状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    /// 正常
    Active,
    /// 禁用
    Disabled,
    /// 错误
    Error {
        /// 错误消息
        message: String,
    },
}

// ============================================================================
// 辅助函数
// ============================================================================

fn default_max_retries() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

// ============================================================================
// 消息编解码
// ============================================================================

/// 消息编码器
pub struct MessageCodec;

impl MessageCodec {
    /// 编码消息为 JSON
    pub fn encode<T: Serialize>(msg: &T) -> crate::ProtocolResult<Vec<u8>> {
        serde_json::to_vec(msg).map_err(crate::ProtocolError::from)
    }

    /// 解码 JSON 为消息
    pub fn decode<T: for<'de> Deserialize<'de>>(data: &[u8]) -> crate::ProtocolResult<T> {
        serde_json::from_slice(data).map_err(crate::ProtocolError::from)
    }

    /// 编码 Hub->Node 消息
    pub fn encode_hub_to_node(msg: &HubToNode) -> crate::ProtocolResult<Vec<u8>> {
        Self::encode(msg)
    }

    /// 解码 Hub->Node 消息
    pub fn decode_hub_to_node(data: &[u8]) -> crate::ProtocolResult<HubToNode> {
        Self::decode(data)
    }

    /// 编码 Node->Hub 消息
    pub fn encode_node_to_hub(msg: &NodeToHub) -> crate::ProtocolResult<Vec<u8>> {
        Self::encode(msg)
    }

    /// 解码 Node->Hub 消息
    pub fn decode_node_to_hub(data: &[u8]) -> crate::ProtocolResult<NodeToHub> {
        Self::decode(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_hub_to_node_serialization() {
        let msg = HubToNode::HeartbeatRequest {
            message_id: MessageId::new(),
            timestamp: Utc::now(),
        };

        let encoded = MessageCodec::encode_hub_to_node(&msg).unwrap();
        let decoded = MessageCodec::decode_hub_to_node(&encoded).unwrap();

        assert_eq!(msg.message_type(), decoded.message_type());
    }

    #[test]
    fn test_hub_to_node_approval_response_serialization() {
        let msg = HubToNode::ApprovalResponse {
            message_id: MessageId::new(),
            request_id: "approval-123".to_string(),
            approved: true,
            responder: "admin".to_string(),
            reason: Some("approved".to_string()),
            responded_at: Utc::now(),
        };

        let encoded = MessageCodec::encode_hub_to_node(&msg).unwrap();
        let decoded = MessageCodec::decode_hub_to_node(&encoded).unwrap();

        assert_eq!(msg.message_type(), decoded.message_type());
        assert_eq!(msg.message_id(), decoded.message_id());
    }

    #[test]
    fn test_node_to_hub_serialization() {
        let msg = NodeToHub::Heartbeat {
            message_id: MessageId::new(),
            node_id: NodeId::new(),
            status: NodeStatus {
                node_id: NodeId::new(),
                online: true,
                current_tasks: 0,
                max_tasks: 5,
                cpu_percent: 10.0,
                memory_mb: 512,
                disk_gb: 50.0,
                network_latency_ms: None,
                last_heartbeat: Utc::now(),
            },
            load: LoadInfo {
                cpu_usage: 0.1,
                memory_usage: 0.2,
                task_count: 0,
                latency_ms: None,
            },
            timestamp: Utc::now(),
        };

        let encoded = MessageCodec::encode_node_to_hub(&msg).unwrap();
        let decoded = MessageCodec::decode_node_to_hub(&encoded).unwrap();

        assert_eq!(msg.message_type(), decoded.message_type());
    }
}
