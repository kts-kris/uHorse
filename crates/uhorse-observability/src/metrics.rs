//! # Prometheus 指标
//!
//! 收集和导出 Prometheus 指标。

use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// 指标收集器
#[derive(Debug)]
pub struct MetricsCollector {
    /// 消息接收计数
    messages_received: AtomicU64,
    /// 消息发送计数
    messages_sent: AtomicU64,
    /// 工具执行次数
    tool_executions: AtomicU64,
    /// 工具执行错误
    tool_errors: AtomicU64,
    /// 活跃会话数
    active_sessions: Arc<RwLock<u64>>,
    /// API 请求计数
    api_requests: AtomicU64,
    /// API 错误计数
    api_errors: AtomicU64,
    /// WebSocket 连接数
    websocket_connections: Arc<RwLock<u64>>,
    /// Agent Loop step 次数
    loop_steps: AtomicU64,
    /// continuation 次数
    continuations: AtomicU64,
    /// approval 等待次数
    approval_waits: AtomicU64,
    /// approval 恢复次数
    approval_resumes: AtomicU64,
    /// planner 重试次数
    planner_retries: AtomicU64,
    /// mailbox 活跃会话数
    mailbox_sessions: Arc<RwLock<u64>>,
    /// 等待工具结果的 turn 数
    waiting_for_tool_turns: Arc<RwLock<u64>>,
    /// 等待审批的 turn 数
    waiting_for_approval_turns: Arc<RwLock<u64>>,
}

#[derive(Debug, Clone, Copy)]
struct MetricsSnapshot {
    messages_received: u64,
    messages_sent: u64,
    tool_executions: u64,
    tool_errors: u64,
    active_sessions: u64,
    api_requests: u64,
    api_errors: u64,
    websocket_connections: u64,
    loop_steps: u64,
    continuations: u64,
    approval_waits: u64,
    approval_resumes: u64,
    planner_retries: u64,
    mailbox_sessions: u64,
    waiting_for_tool_turns: u64,
    waiting_for_approval_turns: u64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            messages_received: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            tool_executions: AtomicU64::new(0),
            tool_errors: AtomicU64::new(0),
            active_sessions: Arc::new(RwLock::new(0)),
            api_requests: AtomicU64::new(0),
            api_errors: AtomicU64::new(0),
            websocket_connections: Arc::new(RwLock::new(0)),
            loop_steps: AtomicU64::new(0),
            continuations: AtomicU64::new(0),
            approval_waits: AtomicU64::new(0),
            approval_resumes: AtomicU64::new(0),
            planner_retries: AtomicU64::new(0),
            mailbox_sessions: Arc::new(RwLock::new(0)),
            waiting_for_tool_turns: Arc::new(RwLock::new(0)),
            waiting_for_approval_turns: Arc::new(RwLock::new(0)),
        }
    }

    /// 记录接收消息
    pub fn inc_messages_received(&self, channel: &str) {
        counter!("uhorse_messages_received_total", "channel" => channel.to_string()).increment(1);
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录发送消息
    pub fn inc_messages_sent(&self, channel: &str) {
        counter!("uhorse_messages_sent_total", "channel" => channel.to_string()).increment(1);
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录工具执行
    pub fn inc_tool_executions(&self, tool: &str) {
        counter!("uhorse_tool_executions_total", "tool" => tool.to_string()).increment(1);
        self.tool_executions.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录工具错误
    pub fn inc_tool_errors(&self, tool: &str, error_type: &str) {
        counter!("uhorse_tool_errors_total", "tool" => tool.to_string(), "error_type" => error_type.to_string()).increment(1);
        self.tool_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录工具执行时间
    pub fn record_tool_execution(&self, tool: &str, duration_ms: u64) {
        histogram!("uhorse_tool_execution_duration_ms", "tool" => tool.to_string())
            .record(duration_ms as f64);
    }

    /// 设置活跃会话数
    pub async fn set_active_sessions(&self, count: u64) {
        gauge!("uhorse_active_sessions").set(count as f64);
        *self.active_sessions.write().await = count;
    }

    /// 增加活跃会话
    pub async fn inc_active_sessions(&self) {
        let mut count = self.active_sessions.write().await;
        *count += 1;
        gauge!("uhorse_active_sessions").set(*count as f64);
    }

    /// 减少活跃会话
    pub async fn dec_active_sessions(&self) {
        let mut count = self.active_sessions.write().await;
        if *count > 0 {
            *count -= 1;
            gauge!("uhorse_active_sessions").set(*count as f64);
        }
    }

    /// 记录 API 请求
    pub fn inc_api_requests(&self, endpoint: &str, method: &str, status: u16) {
        counter!(
            "uhorse_api_requests_total",
            "endpoint" => endpoint.to_string(),
            "method" => method.to_string(),
            "status" => status.to_string()
        )
        .increment(1);
        self.api_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 API 错误
    pub fn inc_api_errors(&self, endpoint: &str, error_type: &str) {
        counter!(
            "uhorse_api_errors_total",
            "endpoint" => endpoint.to_string(),
            "error_type" => error_type.to_string()
        )
        .increment(1);
        self.api_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 API 延迟
    pub fn record_api_latency(&self, endpoint: &str, latency_ms: u64) {
        histogram!("uhorse_api_latency_ms", "endpoint" => endpoint.to_string())
            .record(latency_ms as f64);
    }

    /// WebSocket 连接数变化
    pub async fn set_websocket_connections(&self, count: u64) {
        gauge!("uhorse_websocket_connections").set(count as f64);
        *self.websocket_connections.write().await = count;
    }

    /// 记录 Agent Loop step。
    pub fn inc_loop_steps(&self, stage: &str) {
        counter!("uhorse_loop_steps_total", "stage" => stage.to_string()).increment(1);
        self.loop_steps.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 continuation。
    pub fn inc_continuations(&self, source: &str) {
        counter!("uhorse_continuations_total", "source" => source.to_string()).increment(1);
        self.continuations.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 approval 等待。
    pub fn inc_approval_waits(&self, source: &str) {
        counter!("uhorse_approval_waits_total", "source" => source.to_string()).increment(1);
        self.approval_waits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 approval 恢复。
    pub fn inc_approval_resumes(&self, outcome: &str) {
        counter!("uhorse_approval_resumes_total", "outcome" => outcome.to_string()).increment(1);
        self.approval_resumes.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录 planner 重试。
    pub fn inc_planner_retries(&self, reason: &str) {
        counter!("uhorse_planner_retries_total", "reason" => reason.to_string()).increment(1);
        self.planner_retries.fetch_add(1, Ordering::Relaxed);
    }

    /// 设置 session mailbox 状态。
    pub async fn set_runtime_mailbox_state(
        &self,
        mailbox_sessions: u64,
        waiting_for_tool_turns: u64,
        waiting_for_approval_turns: u64,
    ) {
        gauge!("uhorse_runtime_mailbox_sessions").set(mailbox_sessions as f64);
        gauge!("uhorse_runtime_waiting_for_tool_turns").set(waiting_for_tool_turns as f64);
        gauge!("uhorse_runtime_waiting_for_approval_turns")
            .set(waiting_for_approval_turns as f64);
        *self.mailbox_sessions.write().await = mailbox_sessions;
        *self.waiting_for_tool_turns.write().await = waiting_for_tool_turns;
        *self.waiting_for_approval_turns.write().await = waiting_for_approval_turns;
    }

    /// 增加 WebSocket 连接
    pub async fn inc_websocket_connections(&self) {
        let mut count = self.websocket_connections.write().await;
        *count += 1;
        gauge!("uhorse_websocket_connections").set(*count as f64);
    }

    /// 减少 WebSocket 连接
    pub async fn dec_websocket_connections(&self) {
        let mut count = self.websocket_connections.write().await;
        if *count > 0 {
            *count -= 1;
            gauge!("uhorse_websocket_connections").set(*count as f64);
        }
    }

    /// 记录缓存命中
    pub fn inc_cache_hits(&self, cache_type: &str) {
        counter!("uhorse_cache_hits_total", "type" => cache_type.to_string()).increment(1);
    }

    /// 记录缓存未命中
    pub fn inc_cache_misses(&self, cache_type: &str) {
        counter!("uhorse_cache_misses_total", "type" => cache_type.to_string()).increment(1);
    }

    /// 记录数据库查询时间
    pub fn record_db_query_duration(&self, query_type: &str, duration_ms: u64) {
        histogram!("uhorse_db_query_duration_ms", "type" => query_type.to_string())
            .record(duration_ms as f64);
    }

    async fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            messages_received: self.messages_received.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            tool_executions: self.tool_executions.load(Ordering::Relaxed),
            tool_errors: self.tool_errors.load(Ordering::Relaxed),
            active_sessions: *self.active_sessions.read().await,
            api_requests: self.api_requests.load(Ordering::Relaxed),
            api_errors: self.api_errors.load(Ordering::Relaxed),
            websocket_connections: *self.websocket_connections.read().await,
            loop_steps: self.loop_steps.load(Ordering::Relaxed),
            continuations: self.continuations.load(Ordering::Relaxed),
            approval_waits: self.approval_waits.load(Ordering::Relaxed),
            approval_resumes: self.approval_resumes.load(Ordering::Relaxed),
            planner_retries: self.planner_retries.load(Ordering::Relaxed),
            mailbox_sessions: *self.mailbox_sessions.read().await,
            waiting_for_tool_turns: *self.waiting_for_tool_turns.read().await,
            waiting_for_approval_turns: *self.waiting_for_approval_turns.read().await,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 指标导出器（简化版）
#[derive(Debug)]
pub struct MetricsExporter {
    collector: Arc<MetricsCollector>,
}

impl MetricsExporter {
    /// 创建新的导出器
    pub fn new(collector: Arc<MetricsCollector>) -> Self {
        Self { collector }
    }

    /// 导出 Prometheus 格式指标
    pub async fn export_metrics(&self) -> String {
        let snapshot = self.collector.snapshot().await;

        format!(
            "# HELP uhorse_messages_received_total Total number of messages received by the hub.\n\
# TYPE uhorse_messages_received_total counter\n\
uhorse_messages_received_total {}\n\
# HELP uhorse_messages_sent_total Total number of messages sent by the hub.\n\
# TYPE uhorse_messages_sent_total counter\n\
uhorse_messages_sent_total {}\n\
# HELP uhorse_tool_executions_total Total number of tool executions.\n\
# TYPE uhorse_tool_executions_total counter\n\
uhorse_tool_executions_total {}\n\
# HELP uhorse_tool_errors_total Total number of tool execution errors.\n\
# TYPE uhorse_tool_errors_total counter\n\
uhorse_tool_errors_total {}\n\
# HELP uhorse_active_sessions Current number of active sessions.\n\
# TYPE uhorse_active_sessions gauge\n\
uhorse_active_sessions {}\n\
# HELP uhorse_api_requests_total Total number of API requests handled by the hub.\n\
# TYPE uhorse_api_requests_total counter\n\
uhorse_api_requests_total {}\n\
# HELP uhorse_api_errors_total Total number of API requests that resulted in errors.\n\
# TYPE uhorse_api_errors_total counter\n\
uhorse_api_errors_total {}\n\
# HELP uhorse_websocket_connections Current number of WebSocket connections.\n\
# TYPE uhorse_websocket_connections gauge\n\
uhorse_websocket_connections {}\n\
# HELP uhorse_loop_steps_total Total number of Agent Loop steps.\n\
# TYPE uhorse_loop_steps_total counter\n\
uhorse_loop_steps_total {}\n\
# HELP uhorse_continuations_total Total number of continuation resumes.\n\
# TYPE uhorse_continuations_total counter\n\
uhorse_continuations_total {}\n\
# HELP uhorse_approval_waits_total Total number of approval waits.\n\
# TYPE uhorse_approval_waits_total counter\n\
uhorse_approval_waits_total {}\n\
# HELP uhorse_approval_resumes_total Total number of approval resumes.\n\
# TYPE uhorse_approval_resumes_total counter\n\
uhorse_approval_resumes_total {}\n\
# HELP uhorse_planner_retries_total Total number of planner retries.\n\
# TYPE uhorse_planner_retries_total counter\n\
uhorse_planner_retries_total {}\n\
# HELP uhorse_runtime_mailbox_sessions Current number of session mailboxes tracked by runtime.\n\
# TYPE uhorse_runtime_mailbox_sessions gauge\n\
uhorse_runtime_mailbox_sessions {}\n\
# HELP uhorse_runtime_waiting_for_tool_turns Current number of turns waiting for tool results.\n\
# TYPE uhorse_runtime_waiting_for_tool_turns gauge\n\
uhorse_runtime_waiting_for_tool_turns {}\n\
# HELP uhorse_runtime_waiting_for_approval_turns Current number of turns waiting for approval.\n\
# TYPE uhorse_runtime_waiting_for_approval_turns gauge\n\
uhorse_runtime_waiting_for_approval_turns {}\n",
            snapshot.messages_received,
            snapshot.messages_sent,
            snapshot.tool_executions,
            snapshot.tool_errors,
            snapshot.active_sessions,
            snapshot.api_requests,
            snapshot.api_errors,
            snapshot.websocket_connections,
            snapshot.loop_steps,
            snapshot.continuations,
            snapshot.approval_waits,
            snapshot.approval_resumes,
            snapshot.planner_retries,
            snapshot.mailbox_sessions,
            snapshot.waiting_for_tool_turns,
            snapshot.waiting_for_approval_turns,
        )
    }
}

/// 工具执行计时器
pub struct ToolTimer {
    tool: String,
    start: Instant,
    collector: Arc<MetricsCollector>,
}

impl ToolTimer {
    pub fn new(tool: String, collector: Arc<MetricsCollector>) -> Self {
        collector.inc_tool_executions(&tool);
        Self {
            tool,
            start: Instant::now(),
            collector,
        }
    }
}

impl Drop for ToolTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_millis() as u64;
        self.collector.record_tool_execution(&self.tool, duration);
    }
}

/// API 请求计时器
pub struct ApiTimer {
    endpoint: String,
    method: String,
    start: Instant,
    collector: Arc<MetricsCollector>,
}

impl ApiTimer {
    pub fn new(endpoint: String, method: String, collector: Arc<MetricsCollector>) -> Self {
        Self {
            endpoint,
            method,
            start: Instant::now(),
            collector,
        }
    }

    pub async fn complete_with_status(self, status: u16) {
        let duration = self.start.elapsed().as_millis() as u64;
        self.collector
            .inc_api_requests(&self.endpoint, &self.method, status);
        self.collector.record_api_latency(&self.endpoint, duration);

        if status >= 400 {
            self.collector
                .inc_api_errors(&self.endpoint, status.to_string().as_str());
        }
    }
}

/// 惰境感知的追踪
pub struct Instrumented<T> {
    inner: T,
    collector: Arc<MetricsCollector>,
}

impl<T> Instrumented<T> {
    pub fn new(inner: T, collector: Arc<MetricsCollector>) -> Self {
        Self { inner, collector }
    }

    pub fn collector(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.collector)
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// 结构化日志格式
#[derive(Debug, Serialize, Deserialize)]
pub struct StructuredLog {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub span: Option<SpanInfo>,
    pub message: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpanInfo {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

/// 审计日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub timestamp: u64,
    pub event_type: String,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
    pub session_id: Option<String>,
    pub action: String,
    pub result: AuditResult,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failure,
    Rejected,
    Cancelled,
}

/// 审计日志记录器
#[derive(Debug)]
pub struct AuditLogger {
    logs: Arc<RwLock<Vec<AuditLog>>>,
    #[allow(clippy::derivable_impls)]
    max_logs: usize,
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            max_logs: 10000,
        }
    }

    pub fn with_max_logs(mut self, max: usize) -> Self {
        self.max_logs = max;
        self
    }

    /// 记录审计事件
    pub async fn log(&self, event: AuditLog) {
        // 在使用 event 前克隆需要的字段
        let event_type = event.event_type.clone();
        let user_id = event.user_id.clone();
        let action = event.action.clone();
        let result = event.result.clone();

        let mut logs = self.logs.write().await;
        logs.push(event);

        // 限制日志数量
        if logs.len() > self.max_logs {
            logs.remove(0);
        }

        // 写入 tracing 日志
        tracing::info!(
            event_type = %event_type,
            user_id = ?user_id,
            action = %action,
            result = ?result,
            "Audit event: {}",
            action
        );
    }

    /// 查询审计日志
    pub async fn query_logs(&self, filter: AuditFilter) -> Vec<AuditLog> {
        let logs = self.logs.read().await;

        logs.iter()
            .filter(|log| {
                if let Some(user_id) = &filter.user_id {
                    if log.user_id.as_ref() != Some(user_id) {
                        return false;
                    }
                }
                if let Some(event_type) = &filter.event_type {
                    if &log.event_type != event_type {
                        return false;
                    }
                }
                if let Some(start) = filter.start_time {
                    if log.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = filter.end_time {
                    if log.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }
}

/// 审计日志过滤器
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub user_id: Option<String>,
    pub event_type: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub limit: Option<usize>,
}

/// 暴露中间件指标
#[derive(Debug)]
pub struct ExposeMetrics {
    collector: Arc<MetricsCollector>,
}

impl ExposeMetrics {
    pub fn new(collector: Arc<MetricsCollector>) -> Self {
        Self { collector }
    }

    pub async fn export(&self) -> String {
        MetricsExporter::new(Arc::clone(&self.collector))
            .export_metrics()
            .await
    }
}

/// 系统健康指标
#[derive(Debug, Serialize)]
pub struct HealthMetrics {
    pub status: String,
    pub uptime_seconds: u64,
    pub version: String,
    pub active_sessions: u64,
    pub websocket_connections: u64,
}

/// 系统状态监控
#[derive(Debug)]
pub struct SystemMonitor {
    start_time: std::time::Instant,
    collector: Arc<MetricsCollector>,
}

impl SystemMonitor {
    pub fn new(collector: Arc<MetricsCollector>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            collector,
        }
    }

    pub async fn get_health_metrics(&self) -> HealthMetrics {
        HealthMetrics {
            status: "healthy".to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_sessions: *self.collector.active_sessions.read().await,
            websocket_connections: *self.collector.websocket_connections.read().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_operations() {
        let collector = Arc::new(MetricsCollector::new());

        collector.inc_messages_received("telegram");
        collector.inc_messages_sent("slack");

        assert_eq!(collector.messages_received.load(Ordering::Relaxed), 1);
        assert_eq!(collector.messages_sent.load(Ordering::Relaxed), 1);

        collector.inc_active_sessions().await;
        collector.inc_active_sessions().await;

        assert_eq!(*collector.active_sessions.read().await, 2);
    }

    #[tokio::test]
    async fn test_metrics_exporter_returns_prometheus_payload() {
        let collector = Arc::new(MetricsCollector::new());
        collector.inc_messages_received("telegram");
        collector.inc_messages_sent("slack");
        collector.inc_tool_executions("shell");
        collector.inc_tool_errors("shell", "timeout");
        collector.set_active_sessions(2).await;
        collector.inc_api_requests("/api/health", "GET", 200);
        collector.inc_api_errors("/api/health", "500");
        collector.set_websocket_connections(3).await;
        collector.inc_loop_steps("initial_plan");
        collector.inc_continuations("task_result");
        collector.inc_approval_resumes("approved");
        collector.inc_planner_retries("continuation_error");
        collector.set_runtime_mailbox_state(2, 1, 1).await;

        let output = MetricsExporter::new(collector).export_metrics().await;

        assert!(output.contains("# HELP uhorse_messages_received_total"));
        assert!(output.contains("uhorse_messages_received_total 1"));
        assert!(output.contains("uhorse_tool_errors_total 1"));
        assert!(output.contains("uhorse_active_sessions 2"));
        assert!(output.contains("uhorse_api_requests_total 1"));
        assert!(output.contains("uhorse_websocket_connections 3"));
        assert!(output.contains("uhorse_loop_steps_total 1"));
        assert!(output.contains("uhorse_continuations_total 1"));
        assert!(output.contains("uhorse_approval_resumes_total 1"));
        assert!(output.contains("uhorse_planner_retries_total 1"));
        assert!(output.contains("uhorse_runtime_mailbox_sessions 2"));
        assert!(output.contains("uhorse_runtime_waiting_for_tool_turns 1"));
        assert!(output.contains("uhorse_runtime_waiting_for_approval_turns 1"));
    }

    #[test]
    fn test_tool_timer() {
        let collector = Arc::new(MetricsCollector::new());
        let _timer = ToolTimer::new("test_tool".to_string(), Arc::clone(&collector));
        // Timer drops here and records execution
    }
}
