//! 权限管理
//!
//! 管理节点操作权限，检查命令是否被授权执行

use crate::error::{NodeError, NodeResult};
use crate::workspace::Workspace;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uhorse_protocol::{Command, TaskContext};

/// 权限检查结果
#[derive(Debug, Clone)]
pub enum PermissionResult {
    /// 允许执行
    Allowed,

    /// 拒绝执行
    Denied(String),

    /// 需要审批
    RequiresApproval {
        /// 审批请求 ID
        request_id: String,
        /// 审批原因
        reason: String,
    },
}

/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// 规则 ID
    pub id: String,

    /// 规则名称
    pub name: String,

    /// 描述
    pub description: Option<String>,

    /// 资源模式
    pub resource: ResourcePattern,

    /// 允许的操作
    pub actions: Vec<Action>,

    /// 条件
    #[serde(default)]
    pub conditions: Vec<Condition>,

    /// 是否需要审批
    #[serde(default)]
    pub require_approval: bool,

    /// 优先级（数字越大优先级越高）
    #[serde(default)]
    pub priority: i32,

    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

fn default_true() -> bool {
    true
}

impl PermissionRule {
    /// 创建新的权限规则
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            resource: ResourcePattern::AllowAll,
            actions: vec![Action::Read],
            conditions: vec![],
            require_approval: false,
            priority: 0,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// 设置资源模式
    pub fn with_resource(mut self, resource: ResourcePattern) -> Self {
        self.resource = resource;
        self
    }

    /// 设置操作
    pub fn with_actions(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }

    /// 设置需要审批
    pub fn require_approval(mut self, require: bool) -> Self {
        self.require_approval = require;
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// 检查命令是否匹配此规则
    pub fn matches(&self, command: &Command, context: &TaskContext) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查资源模式
        if !self.resource.matches(command, context) {
            return false;
        }

        // 检查操作
        let required_actions = self.get_required_actions(command);
        for required in required_actions {
            if !self.actions.contains(&required) {
                return false;
            }
        }

        // 检查条件
        for condition in &self.conditions {
            if !condition.evaluate(context) {
                return false;
            }
        }

        true
    }

    /// 获取命令所需的操作
    fn get_required_actions(&self, command: &Command) -> Vec<Action> {
        match command {
            Command::File(file_cmd) => match file_cmd {
                uhorse_protocol::FileCommand::Read { .. } => vec![Action::Read],
                uhorse_protocol::FileCommand::List { .. } => vec![Action::Read],
                uhorse_protocol::FileCommand::Search { .. } => vec![Action::Read],
                uhorse_protocol::FileCommand::Info { .. } => vec![Action::Read],
                uhorse_protocol::FileCommand::Exists { .. } => vec![Action::Read],
                uhorse_protocol::FileCommand::Write { .. } => vec![Action::Write],
                uhorse_protocol::FileCommand::Append { .. } => vec![Action::Write],
                uhorse_protocol::FileCommand::Delete { .. } => vec![Action::Delete],
                uhorse_protocol::FileCommand::Copy { .. } => vec![Action::Read, Action::Write],
                uhorse_protocol::FileCommand::Move { .. } => {
                    vec![Action::Read, Action::Write, Action::Delete]
                }
                uhorse_protocol::FileCommand::CreateDir { .. } => vec![Action::Write],
            },
            Command::Shell(_) => vec![Action::Execute],
            Command::Code(_) => vec![Action::Execute],
            Command::Database(_) => vec![Action::Execute],
            Command::Api(_) => vec![Action::Execute],
            Command::Browser(_) => vec![Action::Execute],
            Command::Skill(_) => vec![Action::Execute],
        }
    }
}

/// 资源模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourcePattern {
    /// 允许所有
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

    /// 命令类型
    CommandType {
        /// 允许的命令类型
        types: Vec<String>,
    },

    /// 组合模式（AND）
    All {
        /// 子模式
        patterns: Vec<ResourcePattern>,
    },

    /// 组合模式（OR）
    Any {
        /// 子模式
        patterns: Vec<ResourcePattern>,
    },
}

impl ResourcePattern {
    /// 检查是否匹配命令
    pub fn matches(&self, command: &Command, _context: &TaskContext) -> bool {
        match self {
            Self::AllowAll => true,

            Self::ExactPath { path } => self.command_involves_path(command, path),

            Self::PathPrefix { prefix } => self.command_path_starts_with(command, prefix),

            Self::Glob { pattern } => self.command_path_matches_glob(command, pattern),

            Self::Regex { pattern } => {
                // 简化处理，实际应该使用正则表达式
                self.command_path_contains(command, pattern)
            }

            Self::CommandType { types } => {
                let cmd_type = format!("{:?}", command.command_type()).to_lowercase();
                types.iter().any(|t| t.to_lowercase() == cmd_type)
            }

            Self::All { patterns } => patterns.iter().all(|p| p.matches(command, _context)),

            Self::Any { patterns } => patterns.iter().any(|p| p.matches(command, _context)),
        }
    }

    fn command_involves_path(&self, command: &Command, target_path: &str) -> bool {
        match command {
            Command::File(file_cmd) => file_cmd.target_path() == target_path,
            Command::Shell(shell_cmd) => shell_cmd
                .cwd
                .as_ref()
                .map(|p| p == target_path)
                .unwrap_or(false),
            _ => false,
        }
    }

    fn command_path_starts_with(&self, command: &Command, prefix: &str) -> bool {
        match command {
            Command::File(file_cmd) => file_cmd.target_path().starts_with(prefix),
            Command::Shell(shell_cmd) => shell_cmd
                .cwd
                .as_ref()
                .map(|p| p.starts_with(prefix))
                .unwrap_or(false),
            _ => false,
        }
    }

    fn command_path_matches_glob(&self, command: &Command, pattern: &str) -> bool {
        match command {
            Command::File(file_cmd) => glob_match::glob_match(pattern, file_cmd.target_path()),
            _ => false,
        }
    }

    fn command_path_contains(&self, command: &Command, substr: &str) -> bool {
        match command {
            Command::File(file_cmd) => file_cmd.target_path().contains(substr),
            _ => false,
        }
    }
}

/// 操作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    /// 工作日限制
    WeekdayRestriction {
        /// 允许的日期（0=周日, 1=周一, ..., 6=周六）
        allowed_days: Vec<u8>,
    },

    /// IP 白名单
    IpWhitelist {
        /// 允许的 IP 地址
        allowed_ips: Vec<String>,
    },
}

impl Condition {
    /// 评估条件
    pub fn evaluate(&self, context: &TaskContext) -> bool {
        match self {
            Self::TimeRange { start, end } => {
                let now = Utc::now();
                let current_time = now.format("%H:%M").to_string();
                current_time >= *start && current_time <= *end
            }

            Self::UserRestriction { allowed_users } => {
                allowed_users.contains(&context.user_id.as_str().to_string())
            }

            Self::SizeLimit { max_bytes } => {
                // 简化处理，实际需要检查命令涉及的数据大小
                true
            }

            Self::WeekdayRestriction { allowed_days } => {
                use chrono::Datelike;
                let now = Utc::now();
                let weekday = now.weekday().num_days_from_sunday() as u8;
                allowed_days.contains(&weekday)
            }

            Self::IpWhitelist { allowed_ips } => {
                // 简化处理，实际需要检查客户端 IP
                true
            }
        }
    }
}

/// 审批请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// 请求 ID
    pub id: String,

    /// 任务 ID
    pub task_id: String,

    /// 命令
    pub command: Command,

    /// 上下文
    pub context: TaskContext,

    /// 请求原因
    pub reason: String,

    /// 请求时间
    pub requested_at: DateTime<Utc>,

    /// 过期时间
    pub expires_at: DateTime<Utc>,

    /// 审批状态
    pub status: ApprovalStatus,
}

/// 审批状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// 等待审批
    Pending,
    /// 已批准
    Approved {
        /// 审批人
        approver: String,
        /// 审批时间
        approved_at: DateTime<Utc>,
    },
    /// 已拒绝
    Rejected {
        /// 审批人
        rejector: String,
        /// 拒绝时间
        rejected_at: DateTime<Utc>,
        /// 拒绝原因
        reason: String,
    },
    /// 已过期
    Expired,
}

/// 权限管理器
#[derive(Debug)]
pub struct PermissionManager {
    /// 权限规则
    rules: Arc<RwLock<Vec<PermissionRule>>>,

    /// 工作空间
    workspace: Arc<Workspace>,

    /// 待审批请求
    pending_approvals: Arc<RwLock<HashMap<String, ApprovalRequest>>>,

    /// 审批超时时间（秒）
    approval_timeout_secs: u64,
}

impl PermissionManager {
    /// 创建新的权限管理器
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            workspace,
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            approval_timeout_secs: 300, // 5 分钟
        }
    }

    /// 添加权限规则
    pub async fn add_rule(&self, rule: PermissionRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
        // 按优先级排序
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// 移除权限规则
    pub async fn remove_rule(&self, id: &str) -> bool {
        let mut rules = self.rules.write().await;
        let len_before = rules.len();
        rules.retain(|r| r.id != id);
        rules.len() < len_before
    }

    /// 检查命令权限
    pub async fn check(&self, command: &Command, context: &TaskContext) -> PermissionResult {
        // 1. 检查工作空间限制
        if !self.check_workspace(command) {
            return PermissionResult::Denied("Operation outside workspace".to_string());
        }

        // 2. 检查权限规则
        let rules = self.rules.read().await;

        for rule in rules.iter() {
            if rule.matches(command, context) {
                if rule.require_approval {
                    let request_id = uuid::Uuid::new_v4().to_string();
                    return PermissionResult::RequiresApproval {
                        request_id: request_id.clone(),
                        reason: format!("Rule '{}' requires approval", rule.name),
                    };
                }
                return PermissionResult::Allowed;
            }
        }

        // 3. 默认拒绝
        PermissionResult::Denied("No matching permission rule".to_string())
    }

    /// 检查工作空间限制
    fn check_workspace(&self, command: &Command) -> bool {
        match command {
            Command::File(file_cmd) => {
                let path = file_cmd.target_path();
                let is_write = matches!(
                    file_cmd,
                    uhorse_protocol::FileCommand::Write { .. }
                        | uhorse_protocol::FileCommand::Append { .. }
                        | uhorse_protocol::FileCommand::Delete { .. }
                        | uhorse_protocol::FileCommand::Move { .. }
                        | uhorse_protocol::FileCommand::CreateDir { .. }
                );
                self.workspace.can_access(path, is_write).unwrap_or(false)
            }
            Command::Shell(shell_cmd) => {
                if let Some(cwd) = &shell_cmd.cwd {
                    self.workspace.can_execute(cwd)
                } else {
                    true
                }
            }
            Command::Code(code_cmd) => {
                if let Some(workdir) = &code_cmd.workdir {
                    self.workspace.can_execute(workdir)
                } else {
                    true
                }
            }
            _ => true, // 其他命令不涉及文件系统
        }
    }

    /// 创建审批请求
    pub async fn create_approval_request(
        &self,
        task_id: String,
        command: Command,
        context: TaskContext,
        reason: String,
    ) -> NodeResult<ApprovalRequest> {
        let now = Utc::now();
        let request = ApprovalRequest {
            id: uuid::Uuid::new_v4().to_string(),
            task_id,
            command,
            context,
            reason,
            requested_at: now,
            expires_at: now + chrono::Duration::seconds(self.approval_timeout_secs as i64),
            status: ApprovalStatus::Pending,
        };

        let mut pending = self.pending_approvals.write().await;
        pending.insert(request.id.clone(), request.clone());

        info!("Created approval request: {}", request.id);
        Ok(request)
    }

    /// 处理审批响应
    pub async fn handle_approval_response(
        &self,
        request_id: &str,
        approved: bool,
        responder: String,
        reason: Option<String>,
    ) -> NodeResult<bool> {
        let mut pending = self.pending_approvals.write().await;

        if let Some(request) = pending.get_mut(request_id) {
            let now = Utc::now();

            // 检查是否过期
            if now > request.expires_at {
                request.status = ApprovalStatus::Expired;
                return Err(NodeError::Permission(
                    "Approval request expired".to_string(),
                ));
            }

            if approved {
                request.status = ApprovalStatus::Approved {
                    approver: responder.clone(),
                    approved_at: now,
                };
                info!("Approval request {} approved by {}", request_id, responder);
                Ok(true)
            } else {
                request.status = ApprovalStatus::Rejected {
                    rejector: responder.clone(),
                    rejected_at: now,
                    reason: reason.unwrap_or_else(|| "No reason provided".to_string()),
                };
                info!("Approval request {} rejected by {}", request_id, responder);
                Ok(false)
            }
        } else {
            Err(NodeError::Permission(format!(
                "Approval request not found: {}",
                request_id
            )))
        }
    }

    /// 获取待审批请求
    pub async fn get_pending_approvals(&self) -> Vec<ApprovalRequest> {
        let pending = self.pending_approvals.read().await;
        pending
            .values()
            .filter(|r| matches!(r.status, ApprovalStatus::Pending))
            .cloned()
            .collect()
    }

    /// 清理过期请求
    pub async fn cleanup_expired(&self) {
        let mut pending = self.pending_approvals.write().await;
        let now = Utc::now();

        pending.retain(|_, request| {
            if matches!(request.status, ApprovalStatus::Pending) && now > request.expires_at {
                debug!("Expiring approval request: {}", request.id);
                return false;
            }
            true
        });
    }

    /// 加载默认规则
    pub async fn load_default_rules(&self) {
        // 规则 1: 允许读取工作空间内的文件
        self.add_rule(
            PermissionRule::new("default-read", "Allow reading files in workspace")
                .with_resource(ResourcePattern::AllowAll)
                .with_actions(vec![Action::Read])
                .require_approval(false)
                .with_priority(1),
        )
        .await;

        // 规则 2: 写入需要审批
        self.add_rule(
            PermissionRule::new("default-write", "Write operations require approval")
                .with_resource(ResourcePattern::AllowAll)
                .with_actions(vec![Action::Write])
                .require_approval(true)
                .with_priority(2),
        )
        .await;

        // 规则 3: 删除需要审批
        self.add_rule(
            PermissionRule::new("default-delete", "Delete operations require approval")
                .with_resource(ResourcePattern::AllowAll)
                .with_actions(vec![Action::Delete])
                .require_approval(true)
                .with_priority(3),
        )
        .await;

        // 规则 4: 执行需要审批
        self.add_rule(
            PermissionRule::new("default-execute", "Execute operations require approval")
                .with_resource(ResourcePattern::AllowAll)
                .with_actions(vec![Action::Execute])
                .require_approval(true)
                .with_priority(4),
        )
        .await;

        info!("Loaded {} default permission rules", 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_context() -> TaskContext {
        TaskContext::new(
            uhorse_protocol::UserId::from_string("test-user"),
            uhorse_protocol::SessionId::new(),
            "test-channel",
        )
    }

    #[tokio::test]
    async fn test_permission_manager() {
        let temp = TempDir::new().unwrap();
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let manager = PermissionManager::new(workspace);

        manager.load_default_rules().await;

        let context = create_test_context();
        let command = Command::File(uhorse_protocol::FileCommand::Exists {
            path: temp.path().to_string_lossy().to_string(),
        });

        let result = manager.check(&command, &context).await;
        assert!(matches!(result, PermissionResult::Allowed));
    }
}
