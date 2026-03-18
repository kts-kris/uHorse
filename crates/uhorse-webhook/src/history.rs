//! Webhook History
//!
//! Webhook 调用历史记录和查询

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Webhook 状态
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookStatus {
    /// 待发送
    Pending,
    /// 成功
    Success,
    /// 失败
    Failed,
    /// 重试中
    Retrying,
    /// 已取消
    Cancelled,
}

impl std::fmt::Display for WebhookStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookStatus::Pending => write!(f, "pending"),
            WebhookStatus::Success => write!(f, "success"),
            WebhookStatus::Failed => write!(f, "failed"),
            WebhookStatus::Retrying => write!(f, "retrying"),
            WebhookStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Webhook 记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRecord {
    /// 记录 ID
    pub id: String,
    /// 端点 URL
    pub endpoint: String,
    /// 事件类型
    pub event_type: String,
    /// 请求载荷
    pub payload: String,
    /// 请求头
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// HTTP 状态码
    pub response_code: Option<u16>,
    /// 响应体
    pub response_body: Option<String>,
    /// 重试次数
    pub retry_count: u32,
    /// 状态
    pub status: WebhookStatus,
    /// 错误消息
    pub error_message: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 持续时间 (毫秒)
    pub duration_ms: Option<u64>,
    /// 租户 ID
    pub tenant_id: String,
}

impl WebhookRecord {
    /// 创建新记录
    pub fn new(
        endpoint: impl Into<String>,
        event_type: impl Into<String>,
        payload: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            endpoint: endpoint.into(),
            event_type: event_type.into(),
            payload: payload.into(),
            headers: HashMap::new(),
            response_code: None,
            response_body: None,
            retry_count: 0,
            status: WebhookStatus::Pending,
            error_message: None,
            created_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            tenant_id: tenant_id.into(),
        }
    }

    /// 添加请求头
    pub fn add_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// 标记成功
    pub fn mark_success(
        mut self,
        response_code: u16,
        response_body: Option<String>,
        duration_ms: u64,
    ) -> Self {
        self.status = WebhookStatus::Success;
        self.response_code = Some(response_code);
        self.response_body = response_body;
        self.completed_at = Some(Utc::now());
        self.duration_ms = Some(duration_ms);
        self
    }

    /// 标记失败
    pub fn mark_failure(mut self, error: impl Into<String>) -> Self {
        self.status = WebhookStatus::Failed;
        self.error_message = Some(error.into());
        self.completed_at = Some(Utc::now());
        self
    }

    /// 标记重试中
    pub fn mark_retrying(mut self) -> Self {
        self.retry_count += 1;
        self.status = WebhookStatus::Retrying;
        self
    }

    /// 检查是否成功
    pub fn is_success(&self) -> bool {
        self.status == WebhookStatus::Success
    }

    /// 检查是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self.status, WebhookStatus::Failed | WebhookStatus::Retrying)
    }
}

/// 查询过滤器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookFilter {
    /// 状态过滤
    pub status: Option<WebhookStatus>,
    /// 事件类型过滤
    pub event_type: Option<String>,
    /// 端点过滤
    pub endpoint: Option<String>,
    /// 租户 ID 过滤
    pub tenant_id: Option<String>,
    /// 开始时间
    pub start_time: Option<DateTime<Utc>>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
}

impl WebhookFilter {
    /// 创建新过滤器
    pub fn new() -> Self {
        Self::default()
    }

    /// 按状态过滤
    pub fn with_status(mut self, status: WebhookStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// 按事件类型过滤
    pub fn with_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    /// 按端点过滤
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// 按租户过滤
    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// 按时间范围过滤
    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// 检查记录是否匹配
    pub fn matches(&self, record: &WebhookRecord) -> bool {
        if let Some(ref status) = self.status {
            if record.status != *status {
                return false;
            }
        }

        if let Some(ref event_type) = self.event_type {
            if record.event_type != *event_type {
                return false;
            }
        }

        if let Some(ref endpoint) = self.endpoint {
            if !record.endpoint.contains(endpoint) {
                return false;
            }
        }

        if let Some(ref tenant_id) = self.tenant_id {
            if record.tenant_id != *tenant_id {
                return false;
            }
        }

        if let Some(start) = self.start_time {
            if record.created_at < start {
                return false;
            }
        }

        if let Some(end) = self.end_time {
            if record.created_at > end {
                return false;
            }
        }

        true
    }
}

/// Webhook 历史
pub struct WebhookHistory {
    /// 记录存储
    records: Arc<RwLock<Vec<WebhookRecord>>>,
    /// 最大记录数
    max_records: usize,
}

impl WebhookHistory {
    /// 创建新的历史存储
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            max_records,
        }
    }

    /// 添加记录
    pub async fn add(&self, record: WebhookRecord) {
        let mut records = self.records.write().await;

        // 检查是否超过最大记录数
        if records.len() >= self.max_records {
            // 删除最旧的记录
            records.remove(0);
        }

        records.push(record);
    }

    /// 更新记录
    pub async fn update(&self, record: &WebhookRecord) {
        let mut records = self.records.write().await;

        if let Some(existing) = records.iter_mut().find(|r| r.id == record.id) {
            *existing = record.clone();
        }
    }

    /// 获取记录
    pub async fn get(&self, id: &str) -> Option<WebhookRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.id == id).cloned()
    }

    /// 查询记录
    pub async fn query(&self, filter: &WebhookFilter) -> Vec<WebhookRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| filter.matches(r))
            .cloned()
            .collect()
    }

    /// 查询最近的记录
    pub async fn recent(&self, limit: usize) -> Vec<WebhookRecord> {
        let records = self.records.read().await;
        records.iter().rev().take(limit).cloned().collect()
    }

    /// 统计
    pub async fn stats(&self) -> WebhookStats {
        let records = self.records.read().await;

        let mut stats = WebhookStats::default();
        stats.total = records.len();

        for record in records.iter() {
            match record.status {
                WebhookStatus::Success => stats.success_count += 1,
                WebhookStatus::Failed => stats.failed_count += 1,
                WebhookStatus::Pending => stats.pending_count += 1,
                WebhookStatus::Retrying => stats.retrying_count += 1,
                WebhookStatus::Cancelled => stats.cancelled_count += 1,
            }

            if let Some(duration) = record.duration_ms {
                stats.total_duration_ms += duration;
                if stats.min_duration_ms == 0 || duration < stats.min_duration_ms {
                    stats.min_duration_ms = duration;
                }
                if duration > stats.max_duration_ms {
                    stats.max_duration_ms = duration;
                }
            }
        }

        if stats.success_count + stats.failed_count > 0 {
            stats.success_rate = (stats.success_count as f64)
                / (stats.success_count + stats.failed_count) as f64
                * 100.0;
        }

        stats
    }

    /// 清理旧记录
    pub async fn cleanup(&self, before: DateTime<Utc>) -> usize {
        let mut records = self.records.write().await;
        let original_len = records.len();
        records.retain(|r| r.created_at >= before);
        original_len - records.len()
    }
}

impl Default for WebhookHistory {
    fn default() -> Self {
        Self::new(10000)
    }
}

/// Webhook 统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookStats {
    /// 总数
    pub total: usize,
    /// 成功数
    pub success_count: usize,
    /// 失败数
    pub failed_count: usize,
    /// 待处理数
    pub pending_count: usize,
    /// 重试中数
    pub retrying_count: usize,
    /// 已取消数
    pub cancelled_count: usize,
    /// 成功率 (%)
    pub success_rate: f64,
    /// 总持续时间 (毫秒)
    pub total_duration_ms: u64,
    /// 最小持续时间 (毫秒)
    pub min_duration_ms: u64,
    /// 最大持续时间 (毫秒)
    pub max_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_record() {
        let record = WebhookRecord::new(
            "https://example.com/webhook",
            "user.created",
            r#"{"user_id": "123"}"#,
            "tenant-001",
        );

        assert!(!record.id.is_empty());
        assert_eq!(record.status, WebhookStatus::Pending);
        assert!(record.response_code.is_none());
    }

    #[tokio::test]
    async fn test_webhook_record_success() {
        let record = WebhookRecord::new(
            "https://example.com/webhook",
            "user.created",
            r#"{"user_id": "123"}"#,
            "tenant-001",
        )
        .mark_success(200, Some(r#"{"status": "ok"}"#.to_string()), 150);

        assert!(record.is_success());
        assert_eq!(record.response_code, Some(200));
        assert!(record.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_webhook_filter() {
        let filter = WebhookFilter::new()
            .with_status(WebhookStatus::Success)
            .with_event_type("user.created");

        let record = WebhookRecord::new(
            "https://example.com/webhook",
            "user.created",
            r#"{}"#,
            "tenant-001",
        )
        .mark_success(200, None, 100);

        assert!(filter.matches(&record));

        let failed_record = WebhookRecord::new(
            "https://example.com/webhook",
            "user.created",
            r#"{}"#,
            "tenant-001",
        )
        .mark_failure("Timeout");

        assert!(!filter.matches(&failed_record));
    }

    #[tokio::test]
    async fn test_webhook_history() {
        let history = WebhookHistory::new(100);

        let record = WebhookRecord::new(
            "https://example.com/webhook",
            "user.created",
            r#"{}"#,
            "tenant-001",
        );

        history.add(record.clone()).await;

        let retrieved = history.get(&record.id).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_webhook_history_query() {
        let history = WebhookHistory::new(100);

        // 添加多条记录
        for i in 0..10 {
            let record = WebhookRecord::new(
                "https://example.com/webhook",
                if i % 2 == 0 {
                    "user.created"
                } else {
                    "user.updated"
                },
                r#"{}"#,
                "tenant-001",
            );
            history.add(record).await;
        }

        let filter = WebhookFilter::new().with_event_type("user.created");
        let results = history.query(&filter).await;

        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_webhook_stats() {
        let history = WebhookHistory::new(100);

        // 添加成功和失败的记录
        for i in 0..10 {
            let mut record =
                WebhookRecord::new("https://example.com/webhook", "test", r#"{}"#, "tenant-001");
            if i < 7 {
                record = record.mark_success(200, None, 100);
            } else {
                record = record.mark_failure("Error");
            }
            history.add(record).await;
        }

        let stats = history.stats().await;
        assert_eq!(stats.total, 10);
        assert_eq!(stats.success_count, 7);
        assert_eq!(stats.failed_count, 3);
        assert_eq!(stats.success_rate, 70.0);
    }
}
