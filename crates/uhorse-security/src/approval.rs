//! # 审批流程
//!
//! 完整的审批流程系统，支持多级审批、条件审批和自动审批规则。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uhorse_core::{Result, UHorseError};
use uuid::Uuid;

/// 审批状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// 待审批
    Pending,
    /// 已批准
    Approved,
    /// 已拒绝
    Rejected,
    /// 已取消
    Cancelled,
    /// 超时
    TimedOut,
}

/// 审批级别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalLevel {
    /// 单级审批
    Single,
    /// 多级审批（顺序）
    Sequential { levels: Vec<String> },
    /// 多级审批（并行）
    Parallel { required: usize },
    /// 条件审批（根据金额/风险等级）
    Conditional { conditions: Vec<ApprovalCondition> },
}

/// 审批条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalCondition {
    pub condition_type: String,
    pub threshold: serde_json::Value,
    pub required_approvers: Vec<String>,
}

/// 审批人
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approver {
    pub user_id: String,
    pub name: String,
    pub role: String,
}

/// 审批决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub approver_id: String,
    pub approved: bool,
    pub comment: Option<String>,
    pub decided_at: u64,
}

/// 审批请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// 请求 ID
    pub id: String,
    /// 操作类型
    pub action: String,
    /// 请求者
    pub requested_by: String,
    /// 审批级别配置
    pub level: ApprovalLevel,
    /// 当前审批状态
    pub status: ApprovalStatus,
    /// 创建时间
    pub created_at: u64,
    /// 过期时间
    pub expires_at: u64,
    /// 审批决策历史
    pub decisions: Vec<ApprovalDecision>,
    /// 所需审批人
    pub required_approvers: Vec<Approver>,
    /// 已审批人数
    pub approved_count: usize,
    /// 拒绝人数
    pub rejected_count: usize,
    /// 审批元数据
    pub metadata: serde_json::Value,
}

impl ApprovalRequest {
    /// 创建新的审批请求
    pub fn new(
        action: String,
        requested_by: String,
        level: ApprovalLevel,
        required_approvers: Vec<Approver>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 默认 24 小时过期
        let expires_at = now + 86400;

        Self {
            id: Uuid::new_v4().to_string(),
            action,
            requested_by,
            level,
            status: ApprovalStatus::Pending,
            created_at: now,
            expires_at,
            decisions: vec![],
            required_approvers,
            approved_count: 0,
            rejected_count: 0,
            metadata: serde_json::json!({}),
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }

    /// 添加审批决策
    pub fn add_decision(&mut self, decision: ApprovalDecision) -> Result<()> {
        // 检查是否已经审批过
        if self
            .decisions
            .iter()
            .any(|d| d.approver_id == decision.approver_id)
        {
            return Err(UHorseError::InternalError(
                "Approver has already decided".to_string(),
            ));
        }

        self.decisions.push(decision.clone());

        if decision.approved {
            self.approved_count += 1;
        } else {
            self.rejected_count += 1;
        }

        Ok(())
    }

    /// 检查是否可以批准
    pub fn can_approve(&self) -> bool {
        match &self.level {
            ApprovalLevel::Single => self.approved_count == 0,
            ApprovalLevel::Sequential { .. } => {
                // 顺序审批：当前级别完成后才能进入下一级
                true
            }
            ApprovalLevel::Parallel { required } => {
                // 并行审批：需要达到批准数量
                self.approved_count < *required
            }
            ApprovalLevel::Conditional { .. } => true,
        }
    }

    /// 检查是否完成
    pub fn is_completed(&self) -> bool {
        match &self.level {
            ApprovalLevel::Single => self.approved_count >= 1 || self.rejected_count >= 1,
            ApprovalLevel::Sequential { levels } => {
                // 顺序审批：需要所有级别都批准
                self.approved_count >= levels.len() || self.rejected_count >= 1
            }
            ApprovalLevel::Parallel { required } => {
                // 并行审批：需要足够的批准数
                self.approved_count >= *required || self.rejected_count >= 1
            }
            ApprovalLevel::Conditional { conditions } => {
                // 条件审批：根据条件判断
                self.rejected_count >= 1 || self.approved_count >= conditions.len()
            }
        }
    }

    /// 更新状态
    pub fn update_status(&mut self) {
        if self.is_expired() {
            self.status = ApprovalStatus::TimedOut;
        } else if self.rejected_count > 0 {
            self.status = ApprovalStatus::Rejected;
        } else if self.is_completed() {
            self.status = ApprovalStatus::Approved;
        }
    }
}

/// 审批规则引擎
#[derive(Debug, Clone)]
pub struct ApprovalRuleEngine {
    /// 自动审批规则
    auto_approve_rules: Vec<ApprovalRule>,
    /// 自动拒绝规则
    auto_reject_rules: Vec<ApprovalRule>,
}

/// 审批规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRule {
    pub rule_name: String,
    pub condition: serde_json::Value,
    pub action: String,
}

impl ApprovalRuleEngine {
    pub fn new() -> Self {
        Self {
            auto_approve_rules: vec![],
            auto_reject_rules: vec![],
        }
    }

    /// 添加自动批准规则
    pub fn add_auto_approve_rule(&mut self, rule: ApprovalRule) {
        self.auto_approve_rules.push(rule);
    }

    /// 添加自动拒绝规则
    pub fn add_auto_reject_rule(&mut self, rule: ApprovalRule) {
        self.auto_reject_rules.push(rule);
    }

    /// 评估是否自动批准
    pub fn evaluate_auto_approve(&self, metadata: &serde_json::Value) -> bool {
        for rule in &self.auto_approve_rules {
            if self.matches_condition(&rule.condition, metadata) {
                debug!("Auto-approve rule matched: {}", rule.rule_name);
                return true;
            }
        }
        false
    }

    /// 评估是否自动拒绝
    pub fn evaluate_auto_reject(&self, metadata: &serde_json::Value) -> bool {
        for rule in &self.auto_reject_rules {
            if self.matches_condition(&rule.condition, metadata) {
                debug!("Auto-reject rule matched: {}", rule.rule_name);
                return true;
            }
        }
        false
    }

    /// 检查条件是否匹配
    fn matches_condition(
        &self,
        condition: &serde_json::Value,
        metadata: &serde_json::Value,
    ) -> bool {
        // 简化实现：检查元数据中是否包含所有条件
        if let Some(obj) = condition.as_object() {
            obj.iter()
                .all(|(key, value)| metadata.get(key) == Some(value))
        } else {
            false
        }
    }
}

impl Default for ApprovalRuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 审批管理器
#[derive(Debug)]
pub struct ApprovalManager {
    /// 待处理的审批请求
    pending: Arc<RwLock<HashMap<String, ApprovalRequest>>>,
    /// 审批历史
    history: Arc<RwLock<HashMap<String, ApprovalRequest>>>,
    /// 规则引擎
    rule_engine: ApprovalRuleEngine,
    /// 用户到审批请求的映射
    user_requests: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            rule_engine: ApprovalRuleEngine::new(),
            user_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置规则引擎
    pub fn with_rule_engine(mut self, engine: ApprovalRuleEngine) -> Self {
        self.rule_engine = engine;
        self
    }

    /// 创建审批请求
    pub async fn create_request(
        &self,
        action: String,
        requested_by: String,
        level: ApprovalLevel,
        required_approvers: Vec<Approver>,
        metadata: serde_json::Value,
    ) -> Result<ApprovalRequest> {
        // 检查自动批准规则
        if self.rule_engine.evaluate_auto_approve(&metadata) {
            info!("Request auto-approved: {}", action);
            // 创建已批准的请求
            let mut request = ApprovalRequest::new(action, requested_by, level, required_approvers);
            request.status = ApprovalStatus::Approved;
            request.approved_count = 1;
            return Ok(request);
        }

        // 检查自动拒绝规则
        if self.rule_engine.evaluate_auto_reject(&metadata) {
            info!("Request auto-rejected: {}", action);
            let mut request = ApprovalRequest::new(action, requested_by, level, required_approvers);
            request.status = ApprovalStatus::Rejected;
            request.rejected_count = 1;
            return Ok(request);
        }

        let mut request =
            ApprovalRequest::new(action.clone(), requested_by, level, required_approvers);
        request.metadata = metadata;

        // 存储请求
        let request_id = request.id.clone();
        let approver_ids: Vec<String> = request
            .required_approvers
            .iter()
            .map(|a| a.user_id.clone())
            .collect();

        self.pending
            .write()
            .await
            .insert(request_id.clone(), request.clone());

        // 更新用户请求映射
        let mut user_requests = self.user_requests.write().await;
        for approver_id in approver_ids {
            user_requests
                .entry(approver_id)
                .or_insert_with(Vec::new)
                .push(request_id.clone());
        }

        info!(
            "Created approval request: {} for action: {}",
            request_id, action
        );

        // 启动过期任务
        let pending = Arc::clone(&self.pending);
        let history = Arc::clone(&self.history);
        let req_id = request_id.clone();
        let expires_at = request.expires_at;

        tokio::spawn(async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if expires_at > now {
                tokio::time::sleep(tokio::time::Duration::from_secs(expires_at - now)).await;
            }

            // 过期后更新状态
            if let Some(mut req) = pending.write().await.get_mut(&req_id) {
                if req.status == ApprovalStatus::Pending {
                    req.status = ApprovalStatus::TimedOut;
                    // 移入历史
                    let req = pending.write().await.remove(&req_id).unwrap();
                    history.write().await.insert(req_id, req);
                }
            }
        });

        Ok(request)
    }

    /// 批准请求
    pub async fn approve_request(
        &self,
        request_id: &str,
        approver_id: String,
        comment: Option<String>,
    ) -> Result<ApprovalRequest> {
        let mut pending = self.pending.write().await;
        let request = pending
            .get_mut(request_id)
            .ok_or_else(|| UHorseError::InternalError("Request not found".to_string()))?;

        if !request.can_approve() {
            return Err(UHorseError::InternalError(
                "Cannot approve at this stage".to_string(),
            ));
        }

        let decision = ApprovalDecision {
            approver_id: approver_id.clone(),
            approved: true,
            comment,
            decided_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        request.add_decision(decision)?;
        request.update_status();

        let updated_request = request.clone();

        // 如果完成，移入历史
        if request.is_completed() {
            let req = pending.remove(request_id).unwrap();
            self.history
                .write()
                .await
                .insert(request_id.to_string(), req);

            // 清理用户请求映射
            let mut user_requests = self.user_requests.write().await;
            for (_, requests) in user_requests.iter_mut() {
                requests.retain(|id| id != request_id);
            }
        }

        info!("Request approved: {} by {}", request_id, approver_id);
        Ok(updated_request)
    }

    /// 拒绝请求
    pub async fn reject_request(
        &self,
        request_id: &str,
        approver_id: String,
        comment: Option<String>,
    ) -> Result<ApprovalRequest> {
        let mut pending = self.pending.write().await;
        let request = pending
            .get_mut(request_id)
            .ok_or_else(|| UHorseError::InternalError("Request not found".to_string()))?;

        let decision = ApprovalDecision {
            approver_id: approver_id.clone(),
            approved: false,
            comment,
            decided_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        request.add_decision(decision)?;
        request.update_status();

        let updated_request = request.clone();

        // 拒绝后立即移入历史
        let req = pending.remove(request_id).unwrap();
        self.history
            .write()
            .await
            .insert(request_id.to_string(), req);

        // 清理用户请求映射
        let mut user_requests = self.user_requests.write().await;
        for (_, requests) in user_requests.iter_mut() {
            requests.retain(|id| id != request_id);
        }

        info!("Request rejected: {} by {}", request_id, approver_id);
        Ok(updated_request)
    }

    /// 取消请求
    pub async fn cancel_request(&self, request_id: &str) -> Result<bool> {
        let mut pending = self.pending.write().await;
        if let Some(mut request) = pending.get_mut(request_id) {
            request.status = ApprovalStatus::Cancelled;
            let req = pending.remove(request_id).unwrap();
            self.history
                .write()
                .await
                .insert(request_id.to_string(), req);
            info!("Request cancelled: {}", request_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取用户的待审批请求
    pub async fn get_user_pending_requests(&self, user_id: &str) -> Result<Vec<ApprovalRequest>> {
        let user_requests = self.user_requests.read().await;
        let pending = self.pending.read().await;

        let request_ids = user_requests.get(user_id).cloned().unwrap_or_default();
        let mut requests = Vec::new();

        for request_id in request_ids {
            if let Some(request) = pending.get(&request_id) {
                requests.push(request.clone());
            }
        }

        Ok(requests)
    }

    /// 获取请求详情
    pub async fn get_request(&self, request_id: &str) -> Result<Option<ApprovalRequest>> {
        let pending = self.pending.read().await;
        let history = self.history.read().await;

        Ok(pending
            .get(request_id)
            .or_else(|| history.get(request_id))
            .cloned())
    }

    /// 列出所有待处理请求
    pub async fn list_pending(&self) -> Result<Vec<ApprovalRequest>> {
        let pending = self.pending.read().await;
        Ok(pending.values().cloned().collect())
    }

    /// 清理过期请求
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut count = 0;
        let mut pending = self.pending.write().await;

        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, r)| r.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            if let Some(mut request) = pending.get_mut(&id) {
                request.status = ApprovalStatus::TimedOut;
                let req = pending.remove(&id).unwrap();
                self.history.write().await.insert(id, req);
                count += 1;
            }
        }

        if count > 0 {
            debug!("Cleaned up {} expired requests", count);
        }

        Ok(count)
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}
