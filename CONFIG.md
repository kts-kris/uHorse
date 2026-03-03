# uHorse 配置指南

## 目录

- [快速开始](#快速开始)
- [配置文件说明](#配置文件说明)
- [环境变量配置](#环境变量配置)
- [各模块配置](#各模块配置)
- [通道配置](#通道配置)
- [LLM 配置](#llm-配置)
- [API 配置](#api-配置)
- [生产环境配置](#生产环境配置)

---

## 快速开始

### 1. 最小配置

创建 `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = []

[database]
path = "./data/uhorse.db"
```

### 2. 启动

```bash
./start.sh
```

### 3. 访问

- API: http://localhost:8080
- 健康检查: http://localhost:8080/health/live
- 指标: http://localhost:8080/metrics

---

## 配置文件说明

### config.toml 主配置文件

```toml
# ==================== 服务器配置 ====================
[server]
# 监听地址
host = "0.0.0.0"              # 0.0.0.0 表示监听所有网卡
port = 8080                   # 服务端口
max_connections = 1000        # 最大连接数

# ==================== 通道配置 ====================
[channels]
# 启用的通道列表
enabled = [
    "telegram",   # Telegram Bot
    "slack",      # Slack Events API
    "discord",    # Discord Bot
    "whatsapp"    # WhatsApp Business API
]

# 各通道详细配置
[channels.telegram]
# Telegram Bot Token
bot_token = "YOUR_TELEGRAM_BOT_TOKEN"
# Webhook 密钥（可选）
webhook_secret = "your_webhook_secret"
# API 超时（秒）
timeout = 30

[channels.slack]
# Slack Bot Token
bot_token = "xoxb-YOUR-SLACK-BOT-TOKEN"
# 签名密钥
signing_secret = "YOUR_SIGNING_SECRET"

[channels.discord]
# Discord Bot Token
bot_token = "MTIzNDU2Nzg5MA.Gh4b2.example"
# Application ID
application_id = "YOUR_APPLICATION_ID"

[channels.whatsapp]
# WhatsApp Access Token
access_token = "YOUR_WHATSAPP_ACCESS_TOKEN"
# Phone Number ID
phone_number_id = "YOUR_PHONE_NUMBER_ID"
# Business Account ID
business_account_id = "YOUR_BUSINESS_ACCOUNT_ID"
# Webhook 验证 Token
webhook_verify_token = "YOUR_VERIFY_TOKEN"

# ==================== 数据库配置 ====================
[database]
# SQLite 数据库文件路径
path = "./data/uhorse.db"
# 连接池大小
pool_size = 10
# 连接超时（秒）
timeout = 30

# PostgreSQL 配置（可选）
[database.postgres]
# 连接 URL
url = "postgresql://uhorse:password@localhost:5432/uhorse"
# 最小连接数
min_connections = 5
# 最大连接数
max_connections = 20
# 连接超时（秒）
connect_timeout = 10
# 空闲超时（秒）
idle_timeout = 600

# ==================== Redis 配置 ====================
[redis]
# Redis 连接 URL
url = "redis://localhost:6379"
# 数据库编号
db = 0
# 连接池大小
pool_size = 10
# 连接超时（秒）
timeout = 5

# ==================== 安全配置 ====================
[security]
# JWT 密钥（必须 32 字符以上）
jwt_secret = "CHANGE_ME_TO_RANDOM_32_CHAR_STRING"
# 访问令牌过期时间（秒）
token_expiry = 86400          # 24 小时
# 刷新令牌过期时间（秒）
refresh_token_expiry = 604800 # 7 天
# 设备配对码过期时间（秒）
pairing_code_expiry = 300     # 5 分钟

# 审批配置
[security.approval]
# 是否启用审批
enabled = true
# 默认审批策略
default_policy = "sequential"  # single, sequential, parallel
# 自动批准规则
auto_approve = [
    { tool = "calculator", max_risk = "low" },
    { tool = "datetime", max_risk = "medium" }
]

# ==================== 日志配置 ====================
[logging]
# 日志级别：trace, debug, info, warn, error
level = "info"
# 日志格式：json, pretty, compact
format = "pretty"
# 日志输出：stdout, file, both
output = "both"
# 日志文件路径
file = "./logs/uhorse.log"
# 日志轮转（MB）
max_size = 100
# 保留日志文件数
max_files = 10

# ==================== 可观测性配置 ====================
[observability]
# Tracing 配置
[observability.tracing]
# 是否启用
enabled = true
# 采样率（0.0 - 1.0）
sample_rate = 0.1
# OTLP 端点（可选）
otlp_endpoint = "http://jaeger:14268/api/traces"

# Metrics 配置
[observability.metrics]
# 是否启用
enabled = true
# 指标端口
port = 9090
# 指标路径
path = "/metrics"

# 审计日志
[observability.audit]
# 是否启用
enabled = true
# 审计日志文件
file = "./logs/audit.log"
# 审计事件过滤器
events = ["auth", "tool_execution", "approval"]

# ==================== 调度器配置 ====================
[scheduler]
# 工作线程数
worker_threads = 4
# 最大并发任务数
max_concurrent_jobs = 100
# 任务队列大小
queue_size = 1000

# ==================== 工具配置 ====================
[tools]
# 工具执行超时（秒）
execution_timeout = 60
# 沙箱配置
[tools.sandbox]
# 是否启用沙箱
enabled = true
# 最大内存（MB）
max_memory = 512
# CPU 限制（0.1 = 10%）
max_cpu = 0.5
# 网络访问控制
allow_network = true
# 允许的主机
allowed_hosts = ["api.example.com"]

# 内置工具
[tools.builtin]
# 启用的内置工具
enabled = [
    "calculator",
    "http",
    "search",
    "datetime",
    "text"
]

# ==================== 会话配置 ====================
[session]
# 会话隔离级别：strict, moderate, loose
default_isolation = "moderate"
# 会话超时（秒）
timeout = 3600               # 1 小时
# 最大会话数
max_sessions = 10000
# 消息历史限制
max_history_messages = 100

# ==================== WebSocket 配置 ====================
[websocket]
# 心跳间隔（秒）
heartbeat_interval = 30
# 心跳超时（秒）
heartbeat_timeout = 90
# 最大消息大小（字节）
max_message_size = 1048576   # 1MB
# 消息队列大小
message_queue_size = 1000
```

---

## 环境变量配置

### .env 文件配置

```bash
# ==================== 服务器配置 ====================
UHORSE_SERVER_HOST=0.0.0.0
UHORSE_SERVER_PORT=8080

# ==================== 数据库配置 ====================
# SQLite（默认）
UHORSE_DATABASE_URL=sqlite://./data/uhorse.db

# PostgreSQL
UHORSE_DATABASE_URL=postgresql://uhorse:password@localhost:5432/uhorse

# ==================== Redis 配置 ====================
UHORSE_REDIS_URL=redis://localhost:6379

# ==================== 日志配置 ====================
RUST_LOG=info                    # trace, debug, info, warn, error
UHORSE_LOG_LEVEL=info

# ==================== 安全配置 ====================
UHORSE_JWT_SECRET=your-secret-key-min-32-char
UHORSE_TOKEN_EXPIRY=86400

# ==================== 通道配置 ====================
UHORSE_TELEGRAM_BOT_TOKEN=
UHORSE_SLACK_BOT_TOKEN=
UHORSE_SLACK_SIGNING_SECRET=
UHORSE_DISCORD_BOT_TOKEN=
UHORSE_WHATSAPP_ACCESS_TOKEN=

# ==================== 数据目录 ====================
UHORSE_DATA_DIR=./data
UHORSE_LOG_DIR=./logs
```

---

## 各模块配置

### 1. Telegram Bot 配置

```toml
[channels.telegram]
bot_token = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
webhook_secret = "your_webhook_secret"

# 或使用环境变量
# export UHORSE_TELEGRAM_BOT_TOKEN="123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
```

**获取 Bot Token：**
1. 与 [@BotFather](https://t.me/botfather) 对话
2. 发送 `/newbot`
3. 按提示设置名称
4. 获得 Token

### 2. Slack 配置

```toml
[channels.slack]
bot_token = "xoxb-YOUR-TOKEN-HERE"
signing_secret = "YOUR_SIGNING_SECRET"

# 或使用环境变量
# export UHORSE_SLACK_BOT_TOKEN="xoxb-YOUR-TOKEN"
# export UHORSE_SLACK_SIGNING_SECRET="YOUR_SECRET"
```

**配置 Slack：**
1. 创建 Slack App: https://api.slack.com/apps
2. 添加 Bot Token Scopes
3. 启用 Events
4. 设置 OAuth Scope
5. 安装到工作区

### 3. Discord 配置

```toml
[channels.discord]
bot_token = "MTIzNDU2Nzg5MA.Gh4b2.example"
application_id = "123456789012345678"

# 或使用环境变量
# export UHORSE_DISCORD_BOT_TOKEN="MTIzNDU2Nzg5MA..."
```

**配置 Discord：**
1. 创建 Discord Application: https://discord.com/developers/applications
2. 创建 Bot
3. 获取 Token
4. 启用 Gateway Intents
5. 生成 Invite URL

### 4. WhatsApp 配置

```toml
[channels.whatsapp]
access_token = "YOUR_ACCESS_TOKEN"
phone_number_id = "YOUR_PHONE_ID"
business_account_id = "YOUR_BA_ID"
webhook_verify_token = "YOUR_VERIFY_TOKEN"

# 或使用环境变量
# export UHORSE_WHATSAPP_ACCESS_TOKEN="..."
```

**配置 WhatsApp：**
1. 创建 Meta App: https://developers.facebook.com/apps
2. 添加 WhatsApp 产品
3. 获取 Access Token
4. 配置 Webhook

---

## API 配置

### API 密钥管理

```bash
# 生成 API 密钥
openssl rand -hex 32

# 设置到环境变量
export UHORSE_API_KEY="your_api_key_here"
```

### CORS 配置

```toml
[server.cors]
# 允许的来源
allowed_origins = [
    "http://localhost:3000",
    "https://example.com"
]
# 允许的方法
allowed_methods = ["GET", "POST", "PUT", "DELETE"]
# 允许的头部
allowed_headers = ["Content-Type", "Authorization"]
# 是否允许凭证
allow_credentials = true
```

### Rate Limiting 配置

```toml
[server.rate_limit]
# 每秒请求数
requests_per_second = 100
# 突口大小
burst = 200
# 白名单 IP
whitelist = ["127.0.0.1"]
```

---

## 生产环境配置

### 生产环境 config.toml 示例

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 10000

[channels]
enabled = ["telegram", "slack", "discord", "whatsapp"]

[database.postgres]
url = "postgresql://uhorse:${DATABASE_PASSWORD}@postgres:5432/uhorse"
min_connections = 10
max_connections = 100
connect_timeout = 5
idle_timeout = 300

[redis]
url = "redis://redis:6379"
pool_size = 50
timeout = 3

[security]
jwt_secret = "${JWT_SECRET}"  # 从环境变量读取
token_expiry = 3600
refresh_token_expiry = 2592000

[logging]
level = "info"
format = "json"
output = "stdout"

[observability.tracing]
enabled = true
sample_rate = 0.01
otlp_endpoint = "http://jaeger:14268/api/traces"

[observability.metrics]
enabled = true
port = 9090

[observability.audit]
enabled = true
file = "/dev/stdout"

[scheduler]
worker_threads = 8
max_concurrent_jobs = 500
queue_size = 5000

[tools.sandbox]
enabled = true
max_memory = 256
max_cpu = 0.5
allow_network = true
allowed_hosts = ["api.openai.com"]
```

### 环境变量（生产）

```bash
# 数据库密码
export DATABASE_PASSWORD="your_secure_password"

# JWT 密钥
export JWT_SECRET="$(openssl rand -hex 32)"

# Telegram Bot
export UHORSE_TELEGRAM_BOT_TOKEN="your_bot_token"

# Redis
export UHORSE_REDIS_URL="redis://redis:6379"

# 日志级别
export RUST_LOG="info"
```

---

## 配置验证

### 检查配置是否有效

```bash
# 验证配置文件
./target/release/uhorse --config config.toml --help

# 查看当前配置
./target/release/uhorse --config config.toml --log-level debug
```

### 常见配置错误

**错误：无法连接数据库**
```toml
# 检查数据库 URL
[database.postgres]
url = "postgresql://user:pass@host:port/db"
```

**错误：通道认证失败**
```bash
# 验证 Bot Token
curl https://api.telegram.org/bot<YOUR_BOT_TOKEN>/getMe
```

---

## 配置最佳实践

### 1. 安全性

- ✅ 使用环境变量存储敏感信息
- ✅ JWT 密钥至少 32 字符
- ✅ 定期轮换密钥
- ✅ 限制 API 访问来源

### 2. 性能

- ✅ 启用 Redis 缓存
- ✅ 配置连接池
- ✅ 调整工作线程数
- ✅ 启用 Metrics 监控

### 3. 可靠性

- ✅ 配置健康检查
- ✅ 启用审计日志
- ✅ 设置合理的超时
- ✅ 配置重试策略

---

## 下一步

- [API 使用指南](API.md)
- [通道集成指南](CHANNELS.md)
- [部署指南](deployments/DEPLOYMENT.md)

## LLM 配置

### 启用 LLM 功能

uHorse 支持集成大语言模型（LLM）来处理用户消息。支持 OpenAI、Azure OpenAI、Anthropic Claude、Google Gemini 等多种服务商。

### 配置方式

#### 方式一：配置文件

在 `config.toml` 中添加：

```toml
[llm]
enabled = true
provider = "openai"  # openai, azure_openai, anthropic, gemini, custom
api_key = "your-api-key"
base_url = "https://api.openai.com/v1"
model = "gpt-3.5-turbo"
temperature = 0.7
max_tokens = 2000
```

#### 方式二：环境变量

在 `.env` 文件中：

```bash
UHORSE_LLM_ENABLED=true
UHORSE_LLM_PROVIDER=openai
UHORSE_LLM_API_KEY=your-api-key
UHORSE_LLM_BASE_URL=https://api.openai.com/v1
UHORSE_LLM_MODEL=gpt-3.5-turbo
```

### 支持的服务商

| 服务商 | provider 值 | base_url |
|--------|------------|----------|
| OpenAI | `openai` | `https://api.openai.com/v1` |
| Azure OpenAI | `azure_openai` | `https://your-resource.openai.azure.com/openai/deployments/your-deployment` |
| Anthropic Claude | `anthropic` | `https://api.anthropic.com/v1` |
| Google Gemini | `gemini` | `https://generativelanguage.googleapis.com/v1beta` |
| 自定义端点 | `custom` | 自定义 URL |

### 配置向导

运行交互式配置向导：

```bash
./target/release/uhorse wizard
```

### 使用效果

启用 LLM 后，用户发送给 Bot 的消息会被转发给 LLM 进行处理，Bot 会将 LLM 的回复返回给用户。

### 示例配置

#### OpenAI GPT-4

```toml
[llm]
enabled = true
provider = "openai"
api_key = "sk-..."
base_url = "https://api.openai.com/v1"
model = "gpt-4"
temperature = 0.7
max_tokens = 2000
```

#### Azure OpenAI

```toml
[llm]
enabled = true
provider = "azure_openai"
api_key = "your-azure-api-key"
base_url = "https://your-resource.openai.azure.com/openai/deployments/gpt-35-turbo"
model = "gpt-35-turbo"
temperature = 0.7
max_tokens = 2000
```

#### Anthropic Claude

```toml
[llm]
enabled = true
provider = "anthropic"
api_key = "sk-ant-..."
base_url = "https://api.anthropic.com/v1"
model = "claude-3-sonnet-20240229"
temperature = 0.7
max_tokens = 2000
```

#### 国内兼容服务

```toml
[llm]
enabled = true
provider = "custom"
api_key = "your-api-key"
base_url = "https://api.example.com/v1"
model = "model-name"
temperature = 0.7
max_tokens = 2000
```

