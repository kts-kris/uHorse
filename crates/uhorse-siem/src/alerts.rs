//! Security Alerts
//!
//! 安全告警管理

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// 告警严重性
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
    /// 严重
    Critical,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Low => write!(f, "low"),
            AlertSeverity::Medium => write!(f, "medium"),
            AlertSeverity::High => write!(f, "high"),
            AlertSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 严重性
    pub severity: AlertSeverity,
    /// 事件类型过滤器
    pub event_types: Vec<String>,
    /// 条件表达式
    pub condition: AlertCondition,
    /// 阈值
    pub threshold: u32,
    /// 时间窗口 (秒)
    pub window_secs: u64,
    /// 是否启用
    pub enabled: bool,
    /// 通知渠道
    pub notification_channels: Vec<String>,
}

impl AlertRule {
    /// 创建新规则
    pub fn new(name: impl Into<String>, severity: AlertSeverity) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            severity,
            event_types: Vec::new(),
            condition: AlertCondition::Always,
            threshold: 1,
            window_secs: 300,
            enabled: true,
            notification_channels: Vec::new(),
        }
    }

    /// 匹配事件
    pub fn matches(&self, event: &crate::export::AuditEvent) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查事件类型
        if !self.event_types.is_empty() && !self.event_types.contains(&event.event_type) {
            return false;
        }

        // 检查条件
        self.condition.evaluate(event)
    }

    /// 设置事件类型过滤器
    pub fn with_event_types(mut self, event_types: Vec<String>) -> Self {
        self.event_types = event_types;
        self
    }

    /// 设置阈值
    pub fn with_threshold(mut self, threshold: u32) -> Self {
        self.threshold = threshold;
        self
    }
}

/// 告警条件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertCondition {
    /// 始终触发
    Always,
    /// 结果匹配
    ResultEquals(String),
    /// Actor 匹配
    ActorEquals(String),
    /// 资源匹配
    ResourceMatches(String),
    /// 操作匹配
    ActionEquals(String),
    /// 自定义表达式
    Custom(String),
    /// 组合条件 (AND)
    And(Vec<AlertCondition>),
    /// 组合条件 (OR)
    Or(Vec<AlertCondition>),
    /// 否定条件
    Not(Box<AlertCondition>),
}

impl AlertCondition {
    /// 评估条件
    pub fn evaluate(&self, event: &crate::export::AuditEvent) -> bool {
        match self {
            AlertCondition::Always => true,
            AlertCondition::ResultEquals(result) => event.result == *result,
            AlertCondition::ActorEquals(actor) => event.actor == *actor,
            AlertCondition::ResourceMatches(pattern) => {
                event.resource.contains(pattern)
            }
            AlertCondition::ActionEquals(action) => event.action == *action,
            AlertCondition::Custom(_) => {
                // 简化实现，实际应解析并执行表达式
                true
            }
            AlertCondition::And(conditions) => {
                conditions.iter().all(|c| c.evaluate(event))
            }
            AlertCondition::Or(conditions) => {
                conditions.iter().any(|c| c.evaluate(event))
            }
            AlertCondition::Not(condition) => !condition.evaluate(event),
        }
    }
}

/// 告警实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// 告警 ID
    pub id: String,
    /// 规则 ID
    pub rule_id: String,
    /// 规则名称
    pub rule_name: String,
    /// 严重性
    pub severity: AlertSeverity,
    /// 触发时间
    pub triggered_at: DateTime<Utc>,
    /// 事件计数
    pub event_count: u32,
    /// 相关事件
    pub events: Vec<crate::export::AuditEvent>,
    /// 状态
    pub status: AlertStatus,
    /// 确认者
    pub acknowledged_by: Option<String>,
    /// 确认时间
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// 备注
    pub notes: Option<String>,
}

/// 告警状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    /// 开启
    Open,
    /// 已确认
    Acknowledged,
    /// 已解决
    Resolved,
    /// 已忽略
    Ignored,
}

/// 告警管理器
pub struct AlertManager {
    /// 告警规则
    rules: Arc<RwLock<HashMap<String, AlertRule>>>,
    /// 活动告警
    alerts: Arc<RwLock<HashMap<String, Alert>>>,
    /// 事件计数器 (规则ID -> 时间窗口内的事件)
    event_counters: Arc<RwLock<HashMap<String, Vec<DateTime<Utc>>>>>,
}

impl AlertManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            alerts: Arc::new(RwLock::new(HashMap::new())),
            event_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加规则
    pub async fn add_rule(&self, rule: AlertRule) {
        let rule_id = rule.id.clone();
        let mut rules = self.rules.write().await;
        rules.insert(rule_id, rule);
    }

    /// 删除规则
    pub async fn remove_rule(&self, rule_id: &str) -> bool {
        let mut rules = self.rules.write().await;
        rules.remove(rule_id).is_some()
    }

    /// 处理事件
    pub async fn process_event(&self, event: &crate::export::AuditEvent) -> Vec<Alert> {
        let mut triggered_alerts = Vec::new();
        let rules = self.rules.read().await;

        for (rule_id, rule) in rules.iter() {
            if !rule.matches(event) {
                continue;
            }

            // 更新事件计数器
            let should_alert = self.update_counter(rule_id, event.timestamp, rule.window_secs).await;

            if should_alert && self.check_threshold(rule_id, rule.threshold).await {
                // 创建告警
                let alert = Alert {
                    id: uuid::Uuid::new_v4().to_string(),
                    rule_id: rule_id.clone(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity,
                    triggered_at: Utc::now(),
                    event_count: 1,
                    events: vec![event.clone()],
                    status: AlertStatus::Open,
                    acknowledged_by: None,
                    acknowledged_at: None,
                    notes: None,
                };

                // 保存告警
                let mut alerts = self.alerts.write().await;
                alerts.insert(alert.id.clone(), alert.clone());

                triggered_alerts.push(alert.clone());

                // 发送通知
                self.send_notifications(&alert, &rule.notification_channels).await;
            }
        }

        triggered_alerts
    }

    /// 更新计数器
    async fn update_counter(&self, rule_id: &str, timestamp: DateTime<Utc>, window_secs: u64) -> bool {
        let mut counters = self.event_counters.write().await;
        let counter = counters.entry(rule_id.to_string()).or_insert_with(Vec::new);

        // 添加新事件时间戳
        counter.push(timestamp);

        // 清理过期时间戳
        let window_start = Utc::now() - Duration::seconds(window_secs as i64);
        counter.retain(|&t| t > window_start);

        true
    }

    /// 检查阈值
    async fn check_threshold(&self, rule_id: &str, threshold: u32) -> bool {
        let counters = self.event_counters.read().await;
        if let Some(counter) = counters.get(rule_id) {
            counter.len() >= threshold as usize
        } else {
            false
        }
    }

    /// 发送通知
    async fn send_notifications(&self, alert: &Alert, channels: &[String]) {
        for channel in channels {
            warn!(
                "Alert notification [{}] {}: {} - {}",
                alert.severity,
                alert.rule_name,
                alert.id,
                channel
            );
            // 实际实现应发送到对应渠道 (email/slack/pagerduty 等)
        }
    }

    /// 确认告警
    pub async fn acknowledge_alert(&self, alert_id: &str, by: &str) -> crate::Result<()> {
        let mut alerts = self.alerts.write().await;

        if let Some(alert) = alerts.get_mut(alert_id) {
            alert.status = AlertStatus::Acknowledged;
            alert.acknowledged_by = Some(by.to_string());
            alert.acknowledged_at = Some(Utc::now());
            Ok(())
        } else {
            Err(crate::SiemError::AlertError(format!("Alert not found: {}", alert_id)))
        }
    }

    /// 解决告警
    pub async fn resolve_alert(&self, alert_id: &str, notes: Option<&str>) -> crate::Result<()> {
        let mut alerts = self.alerts.write().await;

        if let Some(alert) = alerts.get_mut(alert_id) {
            alert.status = AlertStatus::Resolved;
            alert.notes = notes.map(|n| n.to_string());
            Ok(())
        } else {
            Err(crate::SiemError::AlertError(format!("Alert not found: {}", alert_id)))
        }
    }

    /// 获取活动告警
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts
            .values()
            .filter(|a| a.status == AlertStatus::Open || a.status == AlertStatus::Acknowledged)
            .cloned()
            .collect()
    }

    /// 获取所有告警
    pub async fn get_all_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.values().cloned().collect()
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 预定义告警规则
pub fn default_alert_rules() -> Vec<AlertRule> {
    vec![
        // 登录失败检测
        AlertRule {
            id: "login-failure".to_string(),
            name: "Multiple Login Failures".to_string(),
            description: "检测短时间内多次登录失败".to_string(),
            severity: AlertSeverity::Medium,
            event_types: vec!["user.login".to_string()],
            condition: AlertCondition::ResultEquals("failure".to_string()),
            threshold: 5,
            window_secs: 300,
            enabled: true,
            notification_channels: vec!["email".to_string(), "slack".to_string()],
        },
        // 权限拒绝检测
        AlertRule {
            id: "permission-denied".to_string(),
            name: "Permission Denied".to_string(),
            description: "检测权限被拒绝的操作".to_string(),
            severity: AlertSeverity::High,
            event_types: vec!["user.access".to_string()],
            condition: AlertCondition::ResultEquals("denied".to_string()),
            threshold: 3,
            window_secs: 600,
            enabled: true,
            notification_channels: vec!["email".to_string()],
        },
        // 敏感操作检测
        AlertRule {
            id: "sensitive-action".to_string(),
            name: "Sensitive Action".to_string(),
            description: "检测敏感操作 (删除/修改配置等)".to_string(),
            severity: AlertSeverity::High,
            event_types: vec!["config.delete".to_string(), "config.update".to_string()],
            condition: AlertCondition::Always,
            threshold: 1,
            window_secs: 60,
            enabled: true,
            notification_channels: vec!["email".to_string(), "slack".to_string()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_rule_creation() {
        let rule = AlertRule::new("Test Rule", AlertSeverity::High)
            .with_event_types(vec!["user.login".to_string()])
            .with_threshold(5);

        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.severity, AlertSeverity::High);
    }

    #[test]
    fn test_alert_condition_evaluation() {
        let event = crate::export::AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        ).with_result("failure");

        let condition = AlertCondition::ResultEquals("failure".to_string());
        assert!(condition.evaluate(&event));

        let condition = AlertCondition::ResultEquals("success".to_string());
        assert!(!condition.evaluate(&event));
    }

    #[test]
    fn test_alert_condition_and() {
        let event = crate::export::AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        ).with_result("failure");

        let condition = AlertCondition::And(vec![
            AlertCondition::ResultEquals("failure".to_string()),
            AlertCondition::ActionEquals("login".to_string()),
        ]);

        assert!(condition.evaluate(&event));
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let manager = AlertManager::new();

        // 添加规则
        let rule = AlertRule {
            id: "test-rule".to_string(),
            name: "Test Rule".to_string(),
            description: "Test".to_string(),
            severity: AlertSeverity::Medium,
            event_types: vec!["user.login".to_string()],
            condition: AlertCondition::ResultEquals("failure".to_string()),
            threshold: 1,
            window_secs: 300,
            enabled: true,
            notification_channels: vec![],
        };

        manager.add_rule(rule).await;

        // 处理事件
        let event = crate::export::AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        ).with_result("failure");

        let alerts = manager.process_event(&event).await;
        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn test_default_alert_rules() {
        let rules = default_alert_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.id == "login-failure"));
    }
}
