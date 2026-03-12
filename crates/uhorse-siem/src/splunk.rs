//! Splunk HEC Integration
//!
//! 通过 HTTP Event Collector (HEC) 发送日志到 Splunk

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Splunk 配置
#[derive(Debug, Clone)]
pub struct SplunkConfig {
    /// HEC URL
    pub hec_url: String,
    /// HEC Token
    pub hec_token: String,
    /// 索引名称
    pub index: String,
    /// Source
    pub source: String,
    /// Source Type
    pub source_type: String,
    /// 主机名
    pub host: String,
    /// 批量大小
    pub batch_size: usize,
    /// 启用压缩
    pub enable_compression: bool,
}

impl SplunkConfig {
    /// 创建新的配置
    pub fn new(hec_url: &str, hec_token: &str, index: &str) -> Self {
        Self {
            hec_url: hec_url.to_string(),
            hec_token: hec_token.to_string(),
            index: index.to_string(),
            source: "uhorse".to_string(),
            source_type: "uhorse:audit".to_string(),
            host: "localhost".to_string(),
            batch_size: 100,
            enable_compression: true,
        }
    }
}

/// Splunk 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplunkEvent {
    /// 时间戳 (epoch 微秒)
    pub time: i64,
    /// 事件数据
    pub event: serde_json::Value,
    /// Source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Source Type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sourcetype: Option<String>,
    /// 索引
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// 主机
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// 字段
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, serde_json::Value>,
}

impl SplunkEvent {
    /// 创建新事件
    pub fn new(event: serde_json::Value) -> Self {
        Self {
            time: Utc::now().timestamp_micros(),
            event,
            source: None,
            sourcetype: None,
            index: None,
            host: None,
            fields: HashMap::new(),
        }
    }

    /// 从审计事件创建
    pub fn from_audit(audit: &crate::export::AuditEvent) -> Self {
        let mut fields = HashMap::new();
        fields.insert("event_type".to_string(), serde_json::json!(audit.event_type));
        fields.insert("tenant_id".to_string(), serde_json::json!(audit.tenant_id));
        fields.insert("actor".to_string(), serde_json::json!(audit.actor));
        fields.insert("resource".to_string(), serde_json::json!(audit.resource));
        fields.insert("action".to_string(), serde_json::json!(audit.action));
        fields.insert("result".to_string(), serde_json::json!(audit.result));

        if let Some(ref ip) = audit.ip_address {
            fields.insert("ip_address".to_string(), serde_json::json!(ip));
        }

        Self {
            time: audit.timestamp.timestamp_micros(),
            event: serde_json::to_value(audit).unwrap_or_default(),
            source: Some("uhorse".to_string()),
            sourcetype: Some("uhorse:audit".to_string()),
            index: None,
            host: None,
            fields,
        }
    }

    /// 设置 source
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// 设置 source type
    pub fn with_sourcetype(mut self, sourcetype: impl Into<String>) -> Self {
        self.sourcetype = Some(sourcetype.into());
        self
    }

    /// 设置索引
    pub fn with_index(mut self, index: impl Into<String>) -> Self {
        self.index = Some(index.into());
        self
    }

    /// 设置主机
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// 添加字段
    pub fn add_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.fields.insert(key.into(), value);
        self
    }
}

/// Splunk 客户端
pub struct SplunkClient {
    /// 配置
    config: SplunkConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// 事件缓冲
    buffer: Vec<SplunkEvent>,
}

impl SplunkClient {
    /// 创建新的客户端
    pub fn new(config: SplunkConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            buffer: Vec::new(),
        }
    }

    /// 发送单个事件
    pub async fn send_event(&self, event: &SplunkEvent) -> crate::Result<()> {
        let url = format!("{}/services/collector/event", self.config.hec_url);

        let body = serde_json::to_string(event)?;

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Splunk {}", self.config.hec_token))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| crate::SiemError::SplunkError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SiemError::SplunkError(format!(
                "HEC request failed: {}",
                error_text
            )));
        }

        info!("Sent event to Splunk");
        Ok(())
    }

    /// 添加事件到缓冲区
    pub fn buffer_event(&mut self, event: SplunkEvent) {
        self.buffer.push(event);
    }

    /// 刷新缓冲区
    pub async fn flush(&mut self) -> crate::Result<usize> {
        if self.buffer.is_empty() {
            return Ok(0);
        }

        let events = std::mem::take(&mut self.buffer);
        let count = events.len();

        self.send_batch(&events).await?;

        Ok(count)
    }

    /// 发送批量事件
    pub async fn send_batch(&self, events: &[SplunkEvent]) -> crate::Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let url = format!("{}/services/collector/event", self.config.hec_url);

        // 构建批量请求体
        let mut body = String::new();
        for event in events {
            body.push_str(&serde_json::to_string(event)?);
            body.push('\n');
        }

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Splunk {}", self.config.hec_token))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| crate::SiemError::SplunkError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SiemError::SplunkError(format!(
                "HEC batch request failed: {}",
                error_text
            )));
        }

        info!("Sent {} events to Splunk", events.len());
        Ok(())
    }

    /// 发送审计事件
    pub async fn send_audit(&self, audit: &crate::export::AuditEvent) -> crate::Result<()> {
        let mut event = SplunkEvent::from_audit(audit);
        event.source = Some(self.config.source.clone());
        event.sourcetype = Some(self.config.source_type.clone());
        event.index = Some(self.config.index.clone());
        event.host = Some(self.config.host.clone());

        self.send_event(&event).await
    }

    /// 健康检查
    pub async fn health_check(&self) -> crate::Result<bool> {
        let url = format!("{}/services/collector/health", self.config.hec_url);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Splunk {}", self.config.hec_token))
            .send()
            .await
            .map_err(|e| crate::SiemError::SplunkError(format!("Health check failed: {}", e)))?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splunk_event_creation() {
        let event = SplunkEvent::new(serde_json::json!({"message": "test"}))
            .with_source("uhorse")
            .with_sourcetype("uhorse:audit")
            .with_index("main");

        assert!(event.source.is_some());
        assert_eq!(event.source.as_deref(), Some("uhorse"));
    }

    #[test]
    fn test_splunk_event_from_audit() {
        let audit = crate::export::AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        );

        let event = SplunkEvent::from_audit(&audit);

        assert!(event.time > 0);
        assert!(event.fields.contains_key("event_type"));
    }

    #[test]
    fn test_splunk_config() {
        let config = SplunkConfig::new(
            "https://splunk.example.com:8088",
            "token-123",
            "main",
        );

        assert_eq!(config.hec_url, "https://splunk.example.com:8088");
        assert_eq!(config.index, "main");
    }
}
