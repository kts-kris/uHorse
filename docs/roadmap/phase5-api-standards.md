# Phase 5: API 标准体系

## 概述

**目标**: 实现 OpenAPI 3.0 规范、API 版本管理、Rate Limiting，提供标准化 API 文档

**周期**: 3 周

**状态**: 📋 计划中

---

## 1. OpenAPI 3.0 规范

### 1.1 功能需求

- 自动生成 OpenAPI 规范
- Swagger UI 文档
- 客户端 SDK 生成
- API 示例代码

### 1.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-gateway/src/openapi/mod.rs` | OpenAPI 模块 | 📋 |
| `uhorse-gateway/src/openapi/generator.rs` | 规范生成 | 📋 |
| `uhorse-gateway/src/openapi/ui.rs` | Swagger UI | 📋 |
| `scripts/gen-client.sh` | 客户端生成脚本 | 📋 |

### 1.3 API 注解示例

```rust
use utoipa::{OpenApi, ToSchema, IntoParams};
use serde::{Deserialize, Serialize};

/// Agent information
#[derive(ToSchema, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier
    pub id: String,
    /// Agent name
    pub name: String,
    /// Agent description
    pub description: Option<String>,
    /// Tenant ID
    pub tenant_id: String,
    /// Creation timestamp
    pub created_at: i64,
}

/// Create agent request
#[derive(ToSchema, Deserialize)]
pub struct CreateAgentRequest {
    /// Agent name (1-100 characters)
    pub name: String,
    /// Agent description
    pub description: Option<String>,
}

/// List agents query parameters
#[derive(IntoParams, Deserialize)]
pub struct ListAgentsQuery {
    /// Page number (1-based)
    #[param(minimum = 1, default = 1)]
    pub page: Option<u32>,
    /// Page size (1-100)
    #[param(minimum = 1, maximum = 100, default = 20)]
    pub page_size: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/agents",
    params(ListAgentsQuery),
    responses(
        (status = 200, description = "List of agents", body = PaginatedAgents),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Agents"
)]
pub async fn list_agents(
    query: Query<ListAgentsQuery>,
    auth: AuthExtractor,
) -> Result<Json<PaginatedAgents>> {
    // Implementation
}
```

### 1.4 OpenAPI 定义

```rust
#[derive(OpenApi)]
#[openapi(
    info(
        title = "uHorse API",
        version = "3.0.0",
        description = "Enterprise AI Infrastructure Platform API",
        license(name = "MIT"),
        contact(name = "uHorse Team", email = "support@uhorse.io")
    ),
    paths(
        list_agents,
        create_agent,
        get_agent,
        update_agent,
        delete_agent,
        // ... more paths
    ),
    components(
        schemas(
            Agent,
            CreateAgentRequest,
            UpdateAgentRequest,
            PaginatedAgents,
            Error,
            // ... more schemas
        )
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}
```

### 1.5 Swagger UI 配置

```rust
use utoipa_swagger_ui::SwaggerUi;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/docs")
        .url("/openapi.json", ApiDoc::openapi())
        .config(
            SwaggerUiConfig::new()
                .doc_expansion(DocExpansion::List)
                .display_operation_id(false)
                .filter(true)
                .try_it_out_enabled(true)
        )
}
```

---

## 2. API 版本管理

### 2.1 版本策略

- **URL 版本**: `/api/v1/`, `/api/v2/`
- **废弃策略**: 6 个月过渡期
- **向后兼容**: 不破坏现有客户端

### 2.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-gateway/src/versioning/mod.rs` | 版本管理模块 | 📋 |
| `uhorse-gateway/src/versioning/url.rs` | URL 版本 | 📋 |
| `uhorse-gateway/src/versioning/deprecation.rs` | 废弃管理 | 📋 |
| `uhorse-gateway/src/versioning/compat.rs` | 兼容性检查 | 📋 |

### 2.3 版本路由

```rust
use axum::Router;

pub fn build_api_routes() -> Router {
    Router::new()
        // v1 API
        .nest("/api/v1", v1_routes())
        // v2 API
        .nest("/api/v2", v2_routes())
        // 版本信息
        .route("/api/versions", get(list_versions))
}

fn v1_routes() -> Router {
    Router::new()
        .route("/agents", get(list_agents_v1).post(create_agent_v1))
        .route("/agents/:id", get(get_agent_v1).put(update_agent_v1).delete(delete_agent_v1))
        // ... more v1 routes
}

fn v2_routes() -> Router {
    Router::new()
        .route("/agents", get(list_agents_v2).post(create_agent_v2))
        .route("/agents/:id", get(get_agent_v2).put(update_agent_v2).delete(delete_agent_v2))
        // ... more v2 routes
}
```

### 2.4 废弃警告

```rust
/// API 版本信息
pub struct ApiVersion {
    pub version: String,
    pub status: VersionStatus,
    pub release_date: String,
    pub sunset_date: Option<String>,
    pub deprecation_message: Option<String>,
}

pub enum VersionStatus {
    /// 当前稳定版本
    Stable,
    /// 即将废弃
    Deprecated,
    /// 已废弃，仅安全更新
    Maintenance,
    /// 已停止支持
    EOL,
}

/// 废弃响应头
pub fn add_deprecation_headers(
    response: Response,
    version: &ApiVersion,
) -> Response {
    if version.status == VersionStatus::Deprecated {
        response
            .header("Deprecation", "true")
            .header("Sunset", version.sunset_date.as_ref().unwrap())
            .header("Link", format!(
                r#"<{}>; rel="successor-version""#,
                version.successor_version
            ))
    } else {
        response
    }
}
```

### 2.5 版本兼容性

```rust
/// 兼容性检查器
pub struct CompatibilityChecker {
    rules: Vec<CompatibilityRule>,
}

pub enum CompatibilityRule {
    // 必须保持兼容
    Required,
    // 建议保持兼容
    Recommended,
    // 可选
    Optional,
}

impl CompatibilityChecker {
    /// 检查 API 变更是否向后兼容
    pub fn check(&self, old_spec: &OpenApi, new_spec: &OpenApi) -> CompatibilityReport {
        // 检查规则:
        // 1. 不能删除端点
        // 2. 不能删除必填参数
        // 3. 不能改变响应结构 (可选字段可添加)
        // 4. 不能改变认证要求
        // ...
    }
}
```

---

## 3. Rate Limiting

### 3.1 功能需求

- 全局限流
- 用户级限流
- 端点级限流
- 分布式限流 (Redis)

### 3.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-gateway/src/ratelimit/mod.rs` | 限流模块 | 📋 |
| `uhorse-gateway/src/ratelimit/global.rs` | 全局限流 | 📋 |
| `uhorse-gateway/src/ratelimit/user.rs` | 用户限流 | 📋 |
| `uhorse-gateway/src/ratelimit/endpoint.rs` | 端点限流 | 📋 |
| `uhorse-gateway/src/ratelimit/distributed.rs` | 分布式限流 | 📋 |

### 3.3 限流策略

```rust
use governor::{Quota, RateLimiter};

/// 限流配置
pub struct RateLimitConfig {
    /// 全局限流 (请求/秒)
    pub global_rps: u32,
    /// 每用户限流 (请求/秒)
    pub user_rps: u32,
    /// 每端点限流
    pub endpoints: HashMap<String, EndpointLimit>,
}

pub struct EndpointLimit {
    /// 端点路径
    pub path: String,
    /// 限流 (请求/分钟)
    pub rpm: u32,
    /// 突发容量
    pub burst: u32,
}

/// 限流键
pub enum RateLimitKey {
    /// 全局
    Global,
    /// 按 IP
    Ip(String),
    /// 按用户
    User(String),
    /// 按租户
    Tenant(String),
    /// 自定义
    Custom(String),
}
```

### 3.4 中间件实现

```rust
use axum::middleware::Next;
use axum::response::Response;

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    let key = extract_rate_limit_key(&req);

    match limiter.check(&key) {
        Ok(()) => Ok(next.run(req).await),
        Err(_) => {
            let retry_after = limiter.retry_after(&key);
            Err(RateLimitError {
                status: StatusCode::TOO_MANY_REQUESTS,
                retry_after,
                limit: limiter.limit(&key),
                remaining: 0,
            })
        }
    }
}

/// Rate limit error response
pub struct RateLimitError {
    pub status: StatusCode,
    pub retry_after: u64,
    pub limit: u32,
    pub remaining: u32,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (header::RETRY_AFTER, self.retry_after.to_string()),
                ("X-RateLimit-Limit", self.limit.to_string()),
                ("X-RateLimit-Remaining", self.remaining.to_string()),
            ],
            Json(json!({
                "error": "rate_limit_exceeded",
                "message": "Too many requests, please retry later",
                "retry_after": self.retry_after
            })),
        )
            .into_response()
    }
}
```

### 3.5 分布式限流 (Redis)

```rust
use redis::AsyncCommands;

/// Redis-backed distributed rate limiter
pub struct DistributedRateLimiter {
    redis: redis::aio::Connection,
    prefix: String,
}

impl DistributedRateLimiter {
    /// Check rate limit using sliding window algorithm
    pub async fn check(&mut self, key: &str, limit: u32, window_secs: u64) -> Result<bool> {
        let redis_key = format!("{}:{}", self.prefix, key);
        let now = chrono::Utc::now().timestamp();

        // Sliding window algorithm
        let window_start = now - window_secs as i64;

        // Remove old entries
        let _: () = self.redis.zrembyscore(&redis_key, 0, window_start).await?;

        // Count current entries
        let count: i64 = self.redis.zcard(&redis_key).await?;

        if count < limit as i64 {
            // Add new entry
            let _: () = self.redis.zadd(&redis_key, now, now).await?;
            let _: () = self.redis.expire(&redis_key, window_secs as i64).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
```

### 3.6 配置示例

```toml
[rate_limit]
enabled = true
default_rps = 100
default_burst = 200

[rate_limit.global]
rps = 10000
burst = 20000

[rate_limit.user]
rps = 100
burst = 200

[rate_limit.tenant]
# 租户配额
default_rps = 500
tiers = { "enterprise" = 5000, "pro" = 1000, "free" = 100 }

[rate_limit.endpoints]
"/api/v1/agents" = { rpm = 60, burst = 100 }
"/api/v1/chat" = { rpm = 30, burst = 50 }
"/api/v1/messages" = { rpm = 120, burst = 200 }

[rate_limit.redis]
enabled = true
url = "redis://localhost:6379"
key_prefix = "uhorse:ratelimit"
```

---

## 4. 客户端 SDK 生成

### 4.1 支持语言

- TypeScript/JavaScript
- Python
- Go
- Java
- Rust

### 4.2 生成脚本

```bash
#!/bin/bash
# scripts/gen-client.sh

set -e

OPENAPI_URL="http://localhost:8080/openapi.json"
OUTPUT_DIR="./clients"

# TypeScript
openapi-generator-cli generate \
    -i $OPENAPI_URL \
    -g typescript-axios \
    -o $OUTPUT_DIR/typescript \
    --additional-properties=npmName=@uhorse/sdk,npmVersion=3.0.0

# Python
openapi-generator-cli generate \
    -i $OPENAPI_URL \
    -g python \
    -o $OUTPUT_DIR/python \
    --additional-properties=packageName=uhorse_client,packageVersion=3.0.0

# Go
openapi-generator-cli generate \
    -i $OPENAPI_URL \
    -g go \
    -o $OUTPUT_DIR/go \
    --additional-properties=packageName=uhorse,isGoSubmodule=true

# Java
openapi-generator-cli generate \
    -i $OPENAPI_URL \
    -g java \
    -o $OUTPUT_DIR/java \
    --additional-properties=library=okhttp-gson,groupId=io.uhorse,artifactId=uhorse-client

echo "Client SDKs generated in $OUTPUT_DIR"
```

---

## 5. 里程碑验收

### 5.1 功能验收

- [ ] OpenAPI 规范生成正确
- [ ] Swagger UI 可访问
- [ ] API 版本路由正常
- [ ] 废弃警告生效
- [ ] 全局限流工作
- [ ] 用户限流工作
- [ ] 分布式限流工作
- [ ] 客户端 SDK 生成

### 5.2 性能验收

| 指标 | 目标 |
|------|------|
| OpenAPI 生成时间 | < 100ms |
| Swagger UI 加载 | < 2s |
| 限流检查延迟 | < 1ms |
| 分布式限流延迟 | < 5ms |

### 5.3 测试命令

```bash
# OpenAPI 规范
curl http://localhost:8080/openapi.json | jq '.info.version'

# Swagger UI
open http://localhost:8080/docs

# 版本信息
curl http://localhost:8080/api/versions

# Rate Limiting 测试
for i in {1..100}; do
    curl -s -w "%{http_code}\n" http://localhost:8080/api/v1/agents > /dev/null
done
# 应看到 429 状态码

# 限流响应头
curl -I http://localhost:8080/api/v1/agents
# X-RateLimit-Limit: 100
# X-RateLimit-Remaining: 99

# 生成客户端
./scripts/gen-client.sh
```

---

## 6. API 设计规范

### 6.1 URL 设计

```
# 资源命名 (复数)
GET    /api/v1/agents           # 列表
POST   /api/v1/agents           # 创建
GET    /api/v1/agents/:id       # 获取
PUT    /api/v1/agents/:id       # 更新
DELETE /api/v1/agents/:id       # 删除

# 子资源
GET    /api/v1/agents/:id/sessions
POST   /api/v1/agents/:id/sessions

# 动作 (非 CRUD)
POST   /api/v1/agents/:id/start
POST   /api/v1/agents/:id/stop
```

### 6.2 响应格式

```json
// 成功响应
{
    "data": { ... },
    "meta": {
        "page": 1,
        "page_size": 20,
        "total": 100
    }
}

// 错误响应
{
    "error": {
        "code": "VALIDATION_ERROR",
        "message": "Invalid request",
        "details": [
            { "field": "name", "message": "Name is required" }
        ]
    }
}
```

### 6.3 分页参数

```
?page=1&page_size=20
?cursor=abc123&limit=20
```

### 6.4 过滤和排序

```
?status=active
?created_at_gte=2024-01-01
?sort=-created_at,name
```
