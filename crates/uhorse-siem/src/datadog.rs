//! Datadog Logs API Integration
//!
//! 发送日志到 Datadog

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Datadog 配置
#[derive(Debug, Clone)]
pub struct DatadogConfig {
    /// API Key
    pub api_key: String,
    /// App Key (可选)
    pub app_key: Option<String>,
    /// API 端点
    pub api_endpoint: String,
    /// 服务名称
    pub service: String,
    /// 环境名称
    pub env: String,
    /// 批量大小
    pub batch_size: usize,
}

impl DatadogConfig {
    /// 创建新的配置 (US 区域)
    pub fn new(api_key: &str, service: &str, env: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            app_key: None,
            api_endpoint: "https://http-intake.logs.datadoghq.com".to_string(),
            service: service.to_string(),
            env: env.to_string(),
            batch_size: 100,
        }
    }

    /// 创建 EU 区域配置
    pub fn new_eu(api_key: &str, service: &str, env: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            app_key: None,
            api_endpoint: "https://http-intake.logs.datadoghq.eu".to_string(),
            service: service.to_string(),
            env: env.to_string(),
            batch_size: 100,
        }
    }
}

/// Datadog 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatadogLogEntry {
    /// 消息
    pub message: String,
    /// 时间戳
    #[serde(with = "dd_timestamp")]
    pub timestamp: DateTime<Utc>,
    /// 主机
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// 来源
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// 服务
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// 状态
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// 标签
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ddtags: Vec<String>,
    /// 属性
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(flatten)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// 时间戳序列化模块
mod dd_timestamp {
    use chrono::{DateTime, Utc, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(date.timestamp_millis())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ts = i64::deserialize(deserializer)?;
        Utc.timestamp_millis_opt(ts).single().ok_or_else(|| {
            serde::de::Error::custom("Invalid timestamp")
        })
    }
}

impl DatadogLogEntry {
    /// 创建新的日志条目
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            timestamp: Utc::now(),
            host: None,
            source: None,
            service: None,
            status: None,
            ddtags: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// 从审计事件创建
    pub fn from_audit(audit: &crate::export::AuditEvent, service: &str, env: &str) -> Self {
        let mut tags = vec![
            format!("env:{}", env),
            format!("tenant_id:{}", audit.tenant_id),
            format!("event_type:{}", audit.event_type),
        ];

        if let Some(ref ip) = audit.ip_address {
            tags.push(format!("ip_address:{}", ip));
        }

        let mut attributes = HashMap::new();
        attributes.insert("actor".to_string(), serde_json::json!(audit.actor));
        attributes.insert("resource".to_string(), serde_json::json!(audit.resource));
        attributes.insert("action".to_string(), serde_json::json!(audit.action));
        attributes.insert("result".to_string(), serde_json::json!(audit.result));

        Self {
            message: format!("[{}] {} - {} {} on {}",
                audit.event_type,
                audit.actor,
                audit.action,
                audit.result,
                audit.resource
            ),
            timestamp: audit.timestamp,
            host: audit.ip_address.clone(),
            source: Some("uhorse".to_string()),
            service: Some(service.to_string()),
            status: Some(Self::result_to_status(&audit.result)),
            ddtags: tags,
            attributes,
        }
    }

    /// 转换结果到状态
    fn result_to_status(result: &str) -> String {
        match result {
            "success" => "info",
            "failure" | "error" => "error",
            "denied" => "warn",
            "warning" => "warn",
            _ => "info",
        }.to_string()
    }

    /// 设置主机
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// 设置来源
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// 设置状态
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// 添加标签
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.ddtags.push(tag.into());
        self
    }

    /// 添加属性
    pub fn add_attribute(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }
}

/// Datadog 客户端
pub struct DatadogClient {
    /// 配置
    config: DatadogConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// 日志缓冲
    buffer: Vec<DatadogLogEntry>,
}

impl DatadogClient {
    /// 创建新的客户端
    pub fn new(config: DatadogConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            buffer: Vec::new(),
        }
    }

    /// 发送单个日志
    pub async fn send_log(&self, entry: &DatadogLogEntry) -> crate::Result<()> {
        let url = format!("{}/v1/input", self.config.api_endpoint);

        let response = self
            .http_client
            .post(&url)
            .header("DD-API-KEY", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&[entry])
            .send()
            .await
            .map_err(|e| crate::SiemError::DatadogError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SiemError::DatadogError(format!(
                "Logs API request failed: {}",
                error_text
            )));
        }

        info!("Sent log to Datadog");
        Ok(())
    }

    /// 添加日志到缓冲区
    pub fn buffer_log(&mut self, entry: DatadogLogEntry) {
        self.buffer.push(entry);
    }

    /// 刷新缓冲区
    pub async fn flush(&mut self) -> crate::Result<usize> {
        if self.buffer.is_empty() {
            return Ok(0);
        }

        let entries = std::mem::take(&mut self.buffer);
        let count = entries.len();

        self.send_batch(&entries).await?;

        Ok(count)
    }

    /// 发送批量日志
    pub async fn send_batch(&self, entries: &[DatadogLogEntry]) -> crate::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let url = format!("{}/v1/input", self.config.api_endpoint);

        let response = self
            .http_client
            .post(&url)
            .header("DD-API-KEY", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(entries)
            .send()
            .await
            .map_err(|e| crate::SiemError::DatadogError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SiemError::DatadogError(format!(
                "Logs API batch request failed: {}",
                error_text
            )));
        }

        info!("Sent {} logs to Datadog", entries.len());
        Ok(())
    }

    /// 发送审计事件
    pub async fn send_audit(&self, audit: &crate::export::AuditEvent) -> crate::Result<()> {
        let entry = DatadogLogEntry::from_audit(
            audit,
            &self.config.service,
            &self.config.env,
        );

        self.send_log(&entry).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datadog_config() {
        let config = DatadogConfig::new("api-key-123", "uhorse", "production");

        assert_eq!(config.api_key, "api-key-123");
        assert_eq!(config.service, "uhorse");
        assert_eq!(config.env, "production");
    }

    #[test]
    fn test_datadog_log_entry() {
        let entry = DatadogLogEntry::new("Test message")
            .with_host("localhost")
            .with_source("uhorse")
            .with_status("info")
            .add_tag("env:test");

        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.host, Some("localhost".to_string()));
        assert!(entry.ddtags.contains(&"env:test".to_string()));
    }

    #[test]
    fn test_datadog_log_entry_from_audit() {
        let audit = crate::export::AuditEvent::new(
            "user.login",
            "tenant-001",
            "user-123",
            "/auth/login",
            "login",
        ).with_result("success");

        let entry = DatadogLogEntry::from_audit(&audit, "uhorse", "production");

        assert!(entry.message.contains("user.login"));
        assert_eq!(entry.service, Some("uhorse".to_string()));
        assert!(entry.ddtags.iter().any(|t| t.starts_with("tenant_id:")));
    }
}
