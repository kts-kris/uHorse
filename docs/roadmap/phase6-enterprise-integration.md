# Phase 6: 企业集成体系

## 概述

**目标**: 实现 SSO/OAuth2、SIEM 集成、Webhook 增强、第三方系统对接

**周期**: 4 周

**状态**: 📋 计划中

---

## 1. SSO/OAuth2/OIDC

### 1.1 功能需求

- OAuth2 授权服务器
- OpenID Connect (OIDC)
- SAML 2.0 支持
- 企业 IdP 集成 (Okta, Auth0, Azure AD)

### 1.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-sso/src/lib.rs` | SSO 模块定义 | 📋 |
| `uhorse-sso/src/oauth2.rs` | OAuth2 服务器 | 📋 |
| `uhorse-sso/src/oidc.rs` | OIDC 集成 | 📋 |
| `uhorse-sso/src/saml.rs` | SAML 2.0 | 📋 |
| `uhorse-sso/src/idp/mod.rs` | IdP 集成基类 | 📋 |
| `uhorse-sso/src/idp/okta.rs` | Okta 集成 | 📋 |
| `uhorse-sso/src/idp/auth0.rs` | Auth0 集成 | 📋 |
| `uhorse-sso/src/idp/azure.rs` | Azure AD 集成 | 📋 |

### 1.3 OAuth2 授权流程

```rust
use oauth2::*;

/// OAuth2 授权服务器
pub struct OAuth2Server {
    issuer: String,
    clients: HashMap<String, OAuth2Client>,
    code_store: Arc<CodeStore>,
    token_store: Arc<TokenStore>,
}

/// 支持的授权类型
pub enum GrantType {
    /// 授权码
    AuthorizationCode,
    /// 客户端凭证
    ClientCredentials,
    /// 刷新令牌
    RefreshToken,
    /// 密码 (不推荐)
    Password,
}

impl OAuth2Server {
    /// 授权端点
    /// GET /oauth2/authorize
    pub async fn authorize(&self, req: AuthorizationRequest) -> Result<AuthorizationResponse>;

    /// 令牌端点
    /// POST /oauth2/token
    pub async fn token(&self, req: TokenRequest) -> Result<TokenResponse>;

    /// 令牌验证
    /// POST /oauth2/introspect
    pub async fn introspect(&self, token: &str) -> Result<TokenInfo>;

    /// 令牌撤销
    /// POST /oauth2/revoke
    pub async fn revoke(&self, token: &str) -> Result<()>;

    /// JWKS 端点
    /// GET /.well-known/jwks.json
    pub async fn jwks(&self) -> Result<JWKSet>;

    /// OpenID 发现
    /// GET /.well-known/openid-configuration
    pub async fn discovery(&self) -> Result<OpenIDConfiguration>;
}
```

### 1.4 OIDC 实现

```rust
/// OpenID Connect 配置
pub struct OpenIDConfiguration {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
}

/// ID Token
#[derive(Serialize, Deserialize)]
pub struct IdToken {
    pub iss: String,       // 签发者
    pub sub: String,       // 用户唯一标识
    pub aud: String,       // 客户端 ID
    pub exp: i64,          // 过期时间
    pub iat: i64,          // 签发时间
    pub nonce: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}
```

### 1.5 SAML 2.0

```rust
/// SAML 配置
pub struct SamlConfig {
    /// SP 实体 ID
    pub entity_id: String,
    /// ACS 端点
    pub acs_url: String,
    /// IdP 元数据 URL
    pub idp_metadata_url: String,
    /// 证书路径
    pub cert_path: String,
    /// 私钥路径
    pub key_path: String,
}

impl SamlHandler {
    /// 发起 SAML 登录
    /// GET /saml/login
    pub async fn login(&self) -> Result<SamlAuthRequest>;

    /// SAML 回调
    /// POST /saml/acs
    pub async fn acs(&self, response: String) -> Result<SamlAssertion>;

    /// SAML 元数据
    /// GET /saml/metadata
    pub async fn metadata(&self) -> Result<String>;
}
```

### 1.6 IdP 集成

```rust
/// IdP 集成 trait
#[async_trait]
pub trait IdentityProvider: Send + Sync {
    /// 获取 OAuth2 授权 URL
    fn authorization_url(&self, state: &str, redirect_uri: &str) -> String;

    /// 用授权码换取令牌
    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<TokenResponse>;

    /// 获取用户信息
    async fn get_user_info(&self, access_token: &str) -> Result<UserInfo>;

    /// 刷新令牌
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse>;

    /// 撤销令牌
    async fn revoke_token(&self, token: &str) -> Result<()>;
}

/// Okta IdP
pub struct OktaProvider {
    domain: String,
    client_id: String,
    client_secret: String,
}

/// Azure AD IdP
pub struct AzureADProvider {
    tenant_id: String,
    client_id: String,
    client_secret: String,
}

/// Auth0 IdP
pub struct Auth0Provider {
    domain: String,
    client_id: String,
    client_secret: String,
}
```

### 1.7 配置示例

```toml
[sso]
enabled = true
issuer = "https://auth.uhorse.io"
session_timeout = 3600

[sso.oauth2]
access_token_ttl = 3600
refresh_token_ttl = 86400
code_ttl = 300

[sso.clients]
# 内置客户端
internal = { client_id = "uhorse-internal", client_secret = "xxx", redirect_uris = ["http://localhost:8080/callback"] }

[sso.idp.okta]
enabled = true
domain = "your-org.okta.com"
client_id = "xxx"
client_secret = "xxx"

[sso.idp.azure]
enabled = false
tenant_id = "xxx"
client_id = "xxx"
client_secret = "xxx"

[sso.saml]
enabled = false
entity_id = "uhorse"
idp_metadata_url = "https://idp.example.com/metadata"
```

---

## 2. SIEM 集成

### 2.1 功能需求

- 日志导出 (CEF/JSON 格式)
- Splunk HEC 集成
- Datadog Logs API
- 安全告警

### 2.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-siem/src/lib.rs` | SIEM 模块 | 📋 |
| `uhorse-siem/src/export.rs` | 日志导出 | 📋 |
| `uhorse-siem/src/formats.rs` | 格式转换 | 📋 |
| `uhorse-siem/src/splunk.rs` | Splunk 集成 | 📋 |
| `uhorse-siem/src/datadog.rs` | Datadog 集成 | 📋 |
| `uhorse-siem/src/alerts.rs` | 告警规则 | 📋 |

### 2.3 日志导出格式

```rust
/// CEF (Common Event Format)
pub struct CEFEvent {
    /// CEF 版本
    pub version: String,
    /// 设备厂商
    pub device_vendor: String,
    /// 设备产品
    pub device_product: String,
    /// 设备版本
    pub device_version: String,
    /// 签名 ID
    pub signature_id: String,
    /// 事件名称
    pub name: String,
    /// 严重性 (0-10)
    pub severity: u8,
    /// 扩展字段
    pub extensions: HashMap<String, String>,
}

impl CEFEvent {
    pub fn to_string(&self) -> String {
        format!(
            "CEF:{}|{}|{}|{}|{}|{}|{}|{}",
            self.version,
            self.device_vendor,
            self.device_product,
            self.device_version,
            self.signature_id,
            self.name,
            self.severity,
            self.extensions.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .join(" ")
        )
    }
}

/// JSON 格式
#[derive(Serialize)]
pub struct JSONLogEvent {
    pub timestamp: String,
    pub event_type: String,
    pub tenant_id: String,
    pub actor: Option<String>,
    pub resource: String,
    pub action: String,
    pub outcome: String,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
```

### 2.4 Splunk HEC 集成

```rust
use reqwest::Client;

/// Splunk HEC 客户端
pub struct SplunkClient {
    url: String,
    token: String,
    client: Client,
    index: String,
    source: String,
    source_type: String,
}

impl SplunkClient {
    /// 发送事件到 Splunk
    pub async fn send_event(&self, event: &SIEMEvent) -> Result<()> {
        let payload = json!({
            "time": event.timestamp.timestamp(),
            "host": event.host,
            "source": self.source,
            "sourcetype": self.source_type,
            "index": self.index,
            "event": event
        });

        self.client
            .post(&format!("{}/services/collector", self.url))
            .header("Authorization", format!("Splunk {}", self.token))
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }

    /// 批量发送
    pub async fn send_batch(&self, events: &[SIEMEvent]) -> Result<()> {
        let body = events.iter()
            .map(|e| json!({
                "time": e.timestamp.timestamp(),
                "host": e.host,
                "source": self.source,
                "sourcetype": self.source_type,
                "index": self.index,
                "event": e
            }).to_string())
            .join("\n");

        self.client
            .post(&format!("{}/services/collector", self.url))
            .header("Authorization", format!("Splunk {}", self.token))
            .body(body)
            .send()
            .await?;

        Ok(())
    }
}
```

### 2.5 Datadog 集成

```rust
/// Datadog Logs API 客户端
pub struct DatadogClient {
    api_key: String,
    client: Client,
    service: String,
    env: String,
}

impl DatadogClient {
    /// 发送日志到 Datadog
    pub async fn send_log(&self, event: &SIEMEvent) -> Result<()> {
        let payload = json!({
            "ddsource": "uhorse",
            "ddtags": format!("service:{},env:{}", self.service, self.env),
            "hostname": event.host,
            "message": serde_json::to_string(event)?,
            "service": self.service,
        });

        self.client
            .post("https://http-intake.logs.datadoghq.com/v1/input")
            .header("DD-API-KEY", &self.api_key)
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }
}
```

### 2.6 安全告警规则

```yaml
# deployments/siem/alerts/security.yml

# 登录失败告警
- name: MultipleFailedLogins
  condition: |
    count(events[ eventType == "auth.login" and outcome == "failure" ]) >= 5
    within 5 minutes
    groupBy actor
  severity: high
  message: "User {actor} has {count} failed login attempts"
  actions:
    - notify: security-team
    - create_ticket: jira

# 敏感数据访问
- name: SensitiveDataAccess
  condition: |
    events[ eventType == "data.access" and classification == "restricted" ]
  severity: medium
  message: "Sensitive data accessed by {actor}"
  actions:
    - log: audit
    - notify: compliance-team

# 异常 API 调用
- name: AnomalousAPIUsage
  condition: |
    rate(events[ eventType == "api.request" ]) > baseline * 3
  severity: medium
  message: "API usage spike detected: {rate} requests/sec"
  actions:
    - notify: ops-team
    - scale: auto

# 权限提升
- name: PrivilegeEscalation
  condition: |
    events[ eventType == "auth.permission" and action == "grant" and level == "admin" ]
  severity: critical
  message: "Admin privileges granted to {actor}"
  actions:
    - notify: security-team
    - notify: compliance-team
    - create_ticket: jira
```

### 2.7 配置示例

```toml
[siem]
enabled = true
export_format = "json"  # json | cef

[siem.splunk]
enabled = true
url = "https://splunk.example.com:8088"
token = "xxx"
index = "uhorse-security"
source = "uhorse"
sourcetype = "uhorse:audit"

[siem.datadog]
enabled = false
api_key = "xxx"
service = "uhorse"
env = "production"

[siem.export]
# 日志导出 API
enabled = true
path = "/var/log/uhorse/siem-export"
rotation = "daily"
retention_days = 90
```

---

## 3. Webhook 增强

### 3.1 功能需求

- 重试机制 (指数退避)
- 签名验证 (HMAC-SHA256)
- 模板系统
- 历史查询

### 3.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-webhook/src/lib.rs` | Webhook 模块 | 📋 |
| `uhorse-webhook/src/retry.rs` | 重试机制 | 📋 |
| `uhorse-webhook/src/signature.rs` | 签名验证 | 📋 |
| `uhorse-webhook/src/template.rs` | 模板系统 | 📋 |
| `uhorse-webhook/src/history.rs` | 历史记录 | 📋 |
| `uhorse-webhook/src/delivery.rs` | 投递服务 | 📋 |

### 3.3 重试机制

```rust
/// 重试配置
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 退避策略
    pub backoff: BackoffStrategy,
    /// 初始延迟 (毫秒)
    pub initial_delay_ms: u64,
    /// 最大延迟 (毫秒)
    pub max_delay_ms: u64,
    /// 可重试状态码
    pub retryable_status_codes: Vec<u16>,
}

pub enum BackoffStrategy {
    /// 固定延迟
    Fixed,
    /// 线性增长
    Linear,
    /// 指数退避
    Exponential { multiplier: f64 },
    /// 指数退避 + 抖动
    ExponentialJitter { multiplier: f64, jitter: f64 },
}

impl RetryConfig {
    /// 计算下次重试延迟
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = match &self.backoff {
            BackoffStrategy::Fixed => self.initial_delay_ms,
            BackoffStrategy::Linear => self.initial_delay_ms * attempt as u64,
            BackoffStrategy::Exponential { multiplier } => {
                (self.initial_delay_ms as f64 * multiplier.powi(attempt as i32)) as u64
            }
            BackoffStrategy::ExponentialJitter { multiplier, jitter } => {
                let base = (self.initial_delay_ms as f64 * multiplier.powi(attempt as i32)) as u64;
                let jitter = (base as f64 * jitter * rand::random::<f64>()) as u64;
                base + jitter
            }
        };
        Duration::from_millis(delay_ms.min(self.max_delay_ms))
    }
}
```

### 3.4 签名验证

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Webhook 签名器
pub struct WebhookSigner {
    secret: String,
}

impl WebhookSigner {
    /// 创建签名
    pub fn sign(&self, payload: &[u8], timestamp: i64) -> String {
        let message = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(message.as_bytes());

        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }

    /// 验证签名
    pub fn verify(&self, payload: &[u8], timestamp: i64, signature: &str) -> bool {
        let expected = self.sign(payload, timestamp);

        // 常量时间比较防止时序攻击
        let expected_bytes = expected.as_bytes();
        let signature_bytes = signature.as_bytes();

        if expected_bytes.len() != signature_bytes.len() {
            return false;
        }

        expected_bytes
            .iter()
            .zip(signature_bytes.iter())
            .fold(0, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}
```

### 3.5 模板系统

```rust
use handlebars::Handlebars;

/// Webhook 模板
pub struct WebhookTemplate {
    name: String,
    body_template: String,
    headers_template: HashMap<String, String>,
}

impl WebhookTemplate {
    /// 渲染模板
    pub fn render(&self, context: &WebhookContext) -> Result<RenderedWebhook> {
        let handlebars = Handlebars::new();

        let body = handlebars.render_template(&self.body_template, &context)?;

        let headers = self.headers_template
            .iter()
            .map(|(k, v)| {
                let rendered_value = handlebars.render_template(v, &context)?;
                Ok((k.clone(), rendered_value))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(RenderedWebhook { body, headers })
    }
}

/// Webhook 上下文
#[derive(Serialize)]
pub struct WebhookContext {
    pub event_type: String,
    pub event_id: String,
    pub timestamp: i64,
    pub tenant_id: String,
    pub data: serde_json::Value,
}
```

### 3.6 投递服务

```rust
/// Webhook 投递服务
pub struct WebhookDeliveryService {
    client: reqwest::Client,
    retry_config: RetryConfig,
    signer: WebhookSigner,
    history: Arc<WebhookHistory>,
}

impl WebhookDeliveryService {
    /// 发送 Webhook
    pub async fn deliver(&self, webhook: &Webhook) -> Result<DeliveryResult> {
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.try_deliver(webhook).await {
                Ok(response) => {
                    self.history.record_success(webhook, attempt, &response).await;
                    return Ok(DeliveryResult::Success { response, attempts: attempt });
                }
                Err(e) if attempt < self.retry_config.max_attempts => {
                    let delay = self.retry_config.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    self.history.record_failure(webhook, attempt, &e).await;
                    return Ok(DeliveryResult::Failed { error: e.to_string(), attempts: attempt });
                }
            }
        }
    }

    async fn try_deliver(&self, webhook: &Webhook) -> Result<DeliveryResponse> {
        let timestamp = chrono::Utc::now().timestamp();
        let signature = self.signer.sign(webhook.body.as_bytes(), timestamp);

        let response = self.client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Signature", signature)
            .header("X-Webhook-Timestamp", timestamp)
            .header("X-Webhook-Event", &webhook.event_type)
            .body(webhook.body.clone())
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(DeliveryResponse {
                status_code: response.status().as_u16(),
                body: response.text().await?,
            })
        } else {
            Err(anyhow::anyhow!("HTTP {}", response.status()))
        }
    }
}
```

### 3.7 配置示例

```toml
[webhook]
enabled = true
timeout_secs = 30

[webhook.retry]
max_attempts = 5
backoff = "exponential_jitter"
initial_delay_ms = 1000
max_delay_ms = 60000
retryable_status_codes = [408, 429, 500, 502, 503, 504]

[webhook.signature]
algorithm = "hmac-sha256"
secret_env = "WEBHOOK_SECRET"

[webhook.history]
enabled = true
retention_days = 30
max_entries = 100000
```

---

## 4. 第三方集成

### 4.1 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-integration/src/lib.rs` | 集成模块 | 📋 |
| `uhorse-integration/src/jira.rs` | Jira 集成 | 📋 |
| `uhorse-integration/src/github.rs` | GitHub 集成 | 📋 |
| `uhorse-integration/src/slack.rs` | Slack 通知 | 📋 |
| `uhorse-integration/src/pagerduty.rs` | PagerDuty 集成 | 📋 |

### 4.2 Jira 集成

```rust
/// Jira 客户端
pub struct JiraClient {
    url: String,
    auth: JiraAuth,
    client: reqwest::Client,
}

pub enum JiraAuth {
    Basic { username: String, password: String },
    Token { token: String },
}

impl JiraClient {
    /// 创建工单
    pub async fn create_issue(&self, issue: &CreateIssueRequest) -> Result<Issue> {
        let payload = json!({
            "fields": {
                "project": { "key": issue.project_key },
                "summary": issue.summary,
                "description": issue.description,
                "issuetype": { "name": issue.issue_type },
                "priority": { "name": issue.priority },
                "labels": issue.labels,
            }
        });

        let response = self.client
            .post(&format!("{}/rest/api/3/issue", self.url))
            .json(&payload)
            .send()
            .await?;

        Ok(response.json().await?)
    }

    /// 添加评论
    pub async fn add_comment(&self, issue_key: &str, comment: &str) -> Result<()> {
        // Implementation
    }
}
```

### 4.3 GitHub 集成

```rust
/// GitHub 客户端
pub struct GitHubClient {
    token: String,
    client: reqwest::Client,
}

impl GitHubClient {
    /// 创建 Issue
    pub async fn create_issue(&self, owner: &str, repo: &str, issue: &CreateGitHubIssue) -> Result<GitHubIssue> {
        // Implementation
    }

    /// 创建 PR
    pub async fn create_pull_request(&self, owner: &str, repo: &str, pr: &CreatePullRequest) -> Result<PullRequest> {
        // Implementation
    }
}
```

### 4.4 Slack 通知

```rust
/// Slack 客户端
pub struct SlackClient {
    webhook_url: String,
    client: reqwest::Client,
}

impl SlackClient {
    /// 发送消息
    pub async fn send_message(&self, message: &SlackMessage) -> Result<()> {
        let payload = json!({
            "channel": message.channel,
            "username": message.username,
            "icon_emoji": message.icon_emoji,
            "text": message.text,
            "attachments": message.attachments,
        });

        self.client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }
}
```

---

## 5. 里程碑验收

### 5.1 功能验收

- [ ] OAuth2 授权码流程
- [ ] OAuth2 客户端凭证流程
- [ ] OIDC 登录
- [ ] SAML 登录
- [ ] Okta/Azure AD 集成
- [ ] SIEM 日志导出
- [ ] Splunk 集成
- [ ] Webhook 重试
- [ ] Webhook 签名验证
- [ ] Jira/GitHub/Slack 集成

### 5.2 集成测试

```bash
# OAuth2 授权码流程
curl "http://localhost:8080/oauth2/authorize?client_id=xxx&redirect_uri=xxx&response_type=code&scope=openid"

# 令牌交换
curl -X POST http://localhost:8080/oauth2/token \
  -d "grant_type=authorization_code&code=xxx&redirect_uri=xxx"

# SIEM 日志导出
curl http://localhost:8080/api/v1/siem/export?format=cef&start=2024-01-01

# Webhook 测试
curl -X POST http://localhost:8080/api/v1/webhooks \
  -d '{"url": "https://httpbin.org/post", "event": "test", "payload": {}}'

# 签名验证
curl -H "X-Webhook-Signature: sha256=xxx" \
  -H "X-Webhook-Timestamp: 1234567890" \
  http://localhost:8080/webhooks/verify
```

---

## 6. 部署清单

### 6.1 环境变量

```bash
# OAuth2
OAUTH2_ISSUER=https://auth.uhorse.io
OAUTH2_SIGNING_KEY=xxx

# Okta
OKTA_DOMAIN=your-org.okta.com
OKTA_CLIENT_ID=xxx
OKTA_CLIENT_SECRET=xxx

# Azure AD
AZURE_TENANT_ID=xxx
AZURE_CLIENT_ID=xxx
AZURE_CLIENT_SECRET=xxx

# Splunk
SPLUNK_URL=https://splunk.example.com:8088
SPLUNK_TOKEN=xxx

# Webhook
WEBHOOK_SECRET=xxx

# Slack
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/xxx
```

### 6.2 v3.0.0 发布检查

- [ ] 所有 Phase 1-6 功能完成
- [ ] 集成测试通过
- [ ] 性能测试通过
- [ ] 安全审计通过
- [ ] 文档更新完成
- [ ] CHANGELOG 更新
- [ ] 版本号更新
- [ ] Release Notes 准备
