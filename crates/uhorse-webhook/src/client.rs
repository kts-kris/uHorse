//! Webhook Client
//!
//! Webhook 客户端，整合重试、签名、模板、历史功能

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::history::{WebhookHistory, WebhookRecord, WebhookStatus};
use crate::retry::{RetryPolicy, RetryableError};
use crate::signature::{SignatureVerifier, SigningConfig};
use crate::template::{TemplateEngine, WebhookTemplate};

/// Webhook 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// 端点 URL
    pub endpoint: String,
    /// HTTP 方法
    #[serde(default = "default_method")]
    pub method: HttpMethod,
    /// 请求超时 (秒)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// 重试策略
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    /// 签名配置 (可选)
    #[serde(default)]
    pub signing_config: Option<SigningConfig>,
    /// 自定义请求头
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// 模板 (可选)
    #[serde(default)]
    pub template: Option<WebhookTemplate>,
}

fn default_method() -> HttpMethod {
    HttpMethod::Post
}

fn default_timeout() -> u64 {
    30
}

impl WebhookConfig {
    /// 创建新配置
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            method: HttpMethod::Post,
            timeout_secs: 30,
            retry_policy: RetryPolicy::default(),
            signing_config: None,
            headers: HashMap::new(),
            template: None,
        }
    }

    /// 设置 HTTP 方法
    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.method = method;
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// 设置重试策略
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 启用签名
    pub fn with_signing(mut self, secret: impl Into<String>) -> Self {
        self.signing_config = Some(SigningConfig::new(secret));
        self
    }

    /// 添加请求头
    pub fn add_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// 设置模板
    pub fn with_template(mut self, template: WebhookTemplate) -> Self {
        self.template = Some(template);
        self
    }
}

/// HTTP 方法
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// GET
    Get,
    /// POST
    Post,
    /// PUT
    Put,
    /// DELETE
    Delete,
    /// PATCH
    Patch,
}

/// Webhook 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// 事件类型
    pub event_type: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 事件数据
    pub data: serde_json::Value,
    /// 租户 ID
    pub tenant_id: String,
}

impl WebhookEvent {
    /// 创建新事件
    pub fn new(
        event_type: impl Into<String>,
        data: serde_json::Value,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            timestamp: Utc::now(),
            data,
            tenant_id: tenant_id.into(),
        }
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> crate::Result<String> {
        Ok(serde_json::to_string(&self)?)
    }
}

/// Webhook 客户端
pub struct WebhookClient {
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// 历史记录
    history: Arc<WebhookHistory>,
    /// 模板引擎
    template_engine: TemplateEngine,
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WebhookClient {
    /// 创建新客户端
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("Failed to create HTTP client"),
            history: Arc::new(WebhookHistory::default()),
            template_engine: TemplateEngine::new(),
        }
    }

    /// 发送 Webhook
    pub async fn send(
        &self,
        config: &WebhookConfig,
        event: &WebhookEvent,
    ) -> crate::Result<WebhookRecord> {
        let start = Instant::now();

        // 准备载荷
        let payload = self.prepare_payload(config, event)?;

        // 创建历史记录
        let mut record = WebhookRecord::new(
            &config.endpoint,
            &event.event_type,
            &payload,
            &event.tenant_id,
        );

        // 添加自定义请求头
        for (key, value) in &config.headers {
            record = record.add_header(key, value);
        }

        // 添加签名头
        let mut headers = config.headers.clone();
        if let Some(ref signing_config) = config.signing_config {
            let verifier = SignatureVerifier::new(signing_config.clone());
            let signature = verifier.generate_signature_header(payload.as_bytes());
            headers.insert(signing_config.header_name.clone(), signature);
        }

        // 发送请求 (带重试)
        let result = self.send_with_retry(config, &payload, &headers).await;

        // 更新记录
        match result {
            Ok(response) => {
                let duration = start.elapsed().as_millis() as u64;
                record = record.mark_success(
                    response.status().as_u16(),
                    Some(response.text().await.unwrap_or_default()),
                    duration,
                );
                info!("Webhook sent successfully to {}", config.endpoint);
            }
            Err(error) => {
                record = record.mark_failure(error.to_string());
                warn!("Webhook failed: {}", error);
            }
        }

        // 保存历史
        self.history.add(record.clone()).await;

        Ok(record)
    }

    /// 准备载荷
    fn prepare_payload(
        &self,
        config: &WebhookConfig,
        event: &WebhookEvent,
    ) -> crate::Result<String> {
        if let Some(ref template) = config.template {
            // 使用模板
            let mut variables = HashMap::new();
            variables.insert(
                "event_type".to_string(),
                serde_json::json!(event.event_type),
            );
            variables.insert(
                "timestamp".to_string(),
                serde_json::json!(event.timestamp.to_rfc3339()),
            );
            variables.insert("data".to_string(), event.data.clone());
            variables.insert("tenant_id".to_string(), serde_json::json!(event.tenant_id));

            self.template_engine.render(template, &variables)
        } else {
            // 直接使用事件 JSON
            event.to_json()
        }
    }

    /// 带重试的发送
    async fn send_with_retry(
        &self,
        config: &WebhookConfig,
        payload: &str,
        headers: &HashMap<String, String>,
    ) -> crate::Result<reqwest::Response> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            let mut request = match config.method {
                HttpMethod::Get => self.http_client.get(&config.endpoint),
                HttpMethod::Post => self.http_client.post(&config.endpoint),
                HttpMethod::Put => self.http_client.put(&config.endpoint),
                HttpMethod::Delete => self.http_client.delete(&config.endpoint),
                HttpMethod::Patch => self.http_client.patch(&config.endpoint),
            };

            // 添加请求头
            for (key, value) in headers {
                request = request.header(key, value);
            }

            // 设置超时
            request = request.timeout(std::time::Duration::from_secs(config.timeout_secs));

            // 设置请求体
            if config.method != HttpMethod::Get {
                request = request.body(payload.to_string());
            }

            // 发送请求
            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();

                    // 检查是否需要重试
                    if config.retry_policy.retryable_status_codes.contains(&status) {
                        if attempts < config.retry_policy.max_retries {
                            let delay = config.retry_policy.calculate_delay(attempts);
                            warn!("Received status {}, retrying in {:?}", status, delay);
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }

                    if response.status().is_success() {
                        return Ok(response);
                    } else {
                        return Err(crate::WebhookError::HttpError(format!(
                            "HTTP error: {}",
                            status
                        )));
                    }
                }
                Err(error) => {
                    let retryable = if error.is_timeout() || error.is_connect() {
                        true
                    } else if let Some(status) = error.status() {
                        config
                            .retry_policy
                            .retryable_status_codes
                            .contains(&status.as_u16())
                    } else {
                        false
                    };

                    if retryable && attempts < config.retry_policy.max_retries {
                        let delay = config.retry_policy.calculate_delay(attempts);
                        warn!("Request failed: {}, retrying in {:?}", error, delay);
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(crate::WebhookError::HttpError(error.to_string()));
                }
            }
        }
    }

    /// 获取历史记录
    pub fn history(&self) -> &WebhookHistory {
        &self.history
    }

    /// 批量发送
    pub async fn send_batch(
        &self,
        configs: &[WebhookConfig],
        event: &WebhookEvent,
    ) -> Vec<crate::Result<WebhookRecord>> {
        let mut results = Vec::new();

        for config in configs {
            let result = self.send(config, event).await;
            results.push(result);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_webhook_config() {
        let config = WebhookConfig::new("https://example.com/webhook")
            .with_method(HttpMethod::Post)
            .with_timeout(60)
            .add_header("X-Custom", "value");

        assert_eq!(config.endpoint, "https://example.com/webhook");
        assert_eq!(config.method, HttpMethod::Post);
        assert_eq!(config.timeout_secs, 60);
        assert!(config.headers.contains_key("X-Custom"));
    }

    #[test]
    fn test_webhook_event() {
        let event = WebhookEvent::new("user.created", json!({"user_id": "123"}), "tenant-001");

        assert_eq!(event.event_type, "user.created");
        assert_eq!(event.tenant_id, "tenant-001");

        let json = event.to_json().unwrap();
        assert!(json.contains("user.created"));
    }

    #[test]
    fn test_webhook_config_with_signing() {
        let config = WebhookConfig::new("https://example.com/webhook").with_signing("my-secret");

        assert!(config.signing_config.is_some());
    }

    #[tokio::test]
    async fn test_webhook_client_creation() {
        let client = WebhookClient::new();
        assert!(client.history().stats().await.total == 0);
    }
}
