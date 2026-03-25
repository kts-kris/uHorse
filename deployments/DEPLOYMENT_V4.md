# uHorse v4.0 Hub-Node 部署指南

本文档描述 **当前仓库主线** 的部署方式：

- `uhorse-hub`：云端中枢
- `uhorse-node`：本地执行节点
- DingTalk：推荐使用 **Stream 模式**
- LLM：从统一配置 `[llm]` 初始化，支持自定义模型服务商

## 架构概览

```text
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  • HTTP API: /api/health /metrics /api/*    │
│  • WebSocket: /ws                            │
│  • Task scheduling                           │
│  • DingTalk Stream intake                    │
│  • DingTalk result reply                     │
└──────────────────────────────────────────────┘
                      │
                      │ WebSocket
                      ▼
┌──────────────────────────────────────────────┐
│                 uhorse-node                  │
│  • Controlled workspace                      │
│  • File / shell task execution               │
│  • TaskResult return                         │
└──────────────────────────────────────────────┘
```

---

## 目录

- [部署模式](#部署模式)
- [1. 编译二进制](#1-编译二进制)
- [2. 部署 Hub](#2-部署-hub)
- [3. 部署 Node](#3-部署-node)
- [4. 启动与验证](#4-启动与验证)
- [5. DingTalk Stream 配置](#5-dingtalk-stream-配置)
- [6. LLM 与自定义模型服务商配置](#6-llm-与自定义模型服务商配置)
- [7. systemd 示例](#7-systemd-示例)
- [8. 升级建议](#8-升级建议)
- [9. 当前边界](#9-当前边界)

---

## 部署模式

### 模式 A：最小 Hub-Node 闭环

适合：

- 本地或内网快速验证
- 不需要 DingTalk
- 不需要 LLM

### 模式 B：统一配置 Hub + Node

适合：

- 需要 DingTalk Stream
- 需要 LLM
- 需要自定义模型服务商

如果要启用 DingTalk 或 LLM，Hub 必须使用统一配置文件。

---

## 1. 编译二进制

```bash
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs
cargo build --release -p uhorse-hub -p uhorse-node
```

产物：

- `target/release/uhorse-hub`
- `target/release/uhorse-node`

可将它们复制到部署目标机器，例如：

```bash
sudo install -m 755 target/release/uhorse-hub /usr/local/bin/uhorse-hub
sudo install -m 755 target/release/uhorse-node /usr/local/bin/uhorse-node
```

---

## 2. 部署 Hub

### 方式一：最小 legacy `HubConfig`

适合只跑最小调度闭环。

`hub.toml`：

```toml
hub_id = "prod-hub"
bind_address = "0.0.0.0"
port = 8765
max_nodes = 100
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

启动：

```bash
uhorse-hub --config /etc/uhorse/hub.toml --log-level info
```

### 方式二：统一配置

适合 DingTalk / LLM / 自定义模型服务商；如果要镜像 Node Desktop 本地通知到钉钉，也应使用这一配置路径。

示例：

```toml
[server]
host = "0.0.0.0"
port = 8765
max_connections = 1000
request_timeout = 30
read_timeout = 10
write_timeout = 10

[server.health]
enabled = true
path = "/api/health"
verbose = false

[database]
path = "/var/lib/uhorse/uhorse.db"
pool_size = 10
conn_timeout = 30
wal_enabled = true
fk_enabled = true

[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789

[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"

[security]
jwt_secret = "replace-with-random-secret"
token_expiry = 86400
refresh_token_expiry = 2592000
pairing_expiry = 300
approval_enabled = true
pairing_enabled = true

[logging]
level = "info"
format = "pretty"
output = "stdout"
ansi = false
file = true
line = true
target = true

[observability]
service_name = "uhorse-hub"
tracing_enabled = true
metrics_enabled = true
# otlp_endpoint = "http://127.0.0.1:4317"
metrics_port = 9090

[scheduler]
enabled = true
threads = 2
max_concurrent_jobs = 100

[tools]
sandbox_enabled = true
sandbox_timeout = 30
sandbox_max_memory = 512

[llm]
enabled = true
provider = "custom-provider"
api_key = "your_api_key"
base_url = "https://api.example.com/v1"
model = "your-model"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

启动：

```bash
uhorse-hub --config /etc/uhorse/hub.toml --log-level info
```

---

## 3. 部署 Node

`node.toml`：

```toml
name = "office-node-01"
workspace_path = "/Users/you/projects"
heartbeat_interval_secs = 30
status_interval_secs = 60
max_concurrent_tasks = 5
tags = ["default", "macos"]

[connection]
hub_url = "wss://hub.example.com/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = ""
```

启动前可先检查工作目录：

```bash
uhorse-node check --workspace /Users/you/projects
```

启动：

```bash
uhorse-node --config /etc/uhorse/node.toml --log-level info
```

---

## 4. 启动与验证

### 1. 启动 Hub

```bash
uhorse-hub --config /etc/uhorse/hub.toml --log-level info
```

### 2. 启动 Node

```bash
uhorse-node --config /etc/uhorse/node.toml --log-level info
```

### 3. 验证 Hub

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 4. 关键判断标准

- `/api/health` 正常返回
- `/metrics` 可正常抓取 Prometheus 指标
- `/api/nodes` 能看到在线节点
- Node 日志显示已连接 Hub
- Hub 日志显示 `/ws` 连接建立

---

## 5. DingTalk Stream 配置

如果要启用 DingTalk：

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789

[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"
```

当前主推荐是：

- **Stream 模式**
- 入站消息进入 Hub 任务链路
- 任务结果优先通过 `session_webhook` 原路回发；webhook 不可用时回退到群消息或单聊发送
- 当前主线已完成一次真实企业租户验证：不安全请求即时报错，合法请求结果可原路回传

虽然仍保留：

```text
/api/v1/channels/dingtalk/webhook
```

但这不是当前推荐部署模式的主叙事。

---

## 6. LLM 与自定义模型服务商配置

当前 Hub 会从统一配置的 `[llm]` 段初始化 LLM 客户端。

### 内置 provider

当前代码识别：

- `openai`
- `azure_openai`
- `anthropic`
- `gemini`

### 自定义模型服务商

当前也支持把 `provider` 写成任意自定义字符串，例如：

```toml
[llm]
enabled = true
provider = "my-company-llm"
api_key = "your_api_key"
base_url = "https://llm.example.com/v1"
model = "my-model"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

当前行为是：

- 未识别的 provider 会作为 **Custom provider** 处理
- 使用 Bearer Token
- 请求地址为：

```text
{base_url}/chat/completions
```

所以你的自定义服务商需要兼容 OpenAI 风格接口。

---

## 7. systemd 示例

### Hub

`/etc/systemd/system/uhorse-hub.service`：

```ini
[Unit]
Description=uHorse Hub
After=network.target

[Service]
ExecStart=/usr/local/bin/uhorse-hub --config /etc/uhorse/hub.toml --log-level info
Restart=always
RestartSec=5
WorkingDirectory=/var/lib/uhorse

[Install]
WantedBy=multi-user.target
```

### Node

`/etc/systemd/system/uhorse-node.service`：

```ini
[Unit]
Description=uHorse Node
After=network.target

[Service]
ExecStart=/usr/local/bin/uhorse-node --config /etc/uhorse/node.toml --log-level info
Restart=always
RestartSec=5
WorkingDirectory=/var/lib/uhorse

[Install]
WantedBy=multi-user.target
```

启用：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now uhorse-hub
sudo systemctl enable --now uhorse-node
```

---

## 8. 升级建议

### Hub

1. 备份 `hub.toml`
2. 替换 `uhorse-hub` 二进制
3. 重启服务
4. 验证 `/api/health`
5. 验证 `/api/nodes`

### Node

1. 备份 `node.toml`
2. 替换 `uhorse-node` 二进制
3. 重启服务
4. 验证是否重新连上 Hub

---

## 9. 当前边界

部署时需要特别注意以下边界：

- 当前统一配置并不会覆盖所有 Hub 专属调度字段
- `server.health.path` 需要与当前主线路由保持一致，推荐直接配置为 `/api/health`
- `deployments/k8s/base/*` 仍偏旧单体视角，不应直接当作当前 v4.0 生产模板
- 若要在你自己的环境复现 DingTalk 最后一跳，仍需要准备你自己的真实企业凭据

如果你要做当前主线部署，请把这份文档和 [../CONFIG.md](../CONFIG.md) 一起看。
