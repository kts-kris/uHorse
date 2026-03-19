# uHorse 配置指南

本文档只描述**当前仓库代码实际读取到的配置结构**，重点覆盖：

- `uhorse-hub` 的两种配置模式
- `uhorse-node` 的 `node.toml`
- DingTalk Stream 和 LLM 的真实字段

## 目录

- [配置模式总览](#配置模式总览)
- [Hub 配置](#hub-配置)
- [Node 配置](#node-配置)
- [DingTalk Stream 配置](#dingtalk-stream-配置)
- [LLM 配置](#llm-配置)
- [验证命令](#验证命令)

---

## 配置模式总览

`uhorse-hub` 当前有 **两种配置模式**。

### 模式 1：统一配置

适用场景：

- 需要启用 DingTalk
- 需要启用 LLM
- 需要使用 `uhorse-config` 的统一结构

入口命令：

```bash
./target/release/uhorse-hub --config hub.toml
```

识别方式：

只要配置文件中出现以下任一段落，Hub 就会按统一配置解析：

- `[server]`
- `[database]`
- `[channels]`
- `[security]`
- `[logging]`
- `[observability]`
- `[scheduler]`
- `[tools]`
- `[llm]`

注意：当前代码里，统一配置会直接驱动：

- Hub 监听地址与端口（来自 `[server]`）
- DingTalk 初始化（来自 `[channels.dingtalk]`）
- LLM 初始化（来自 `[llm]`）

但 **`max_nodes`、`heartbeat_timeout_secs`、`task_timeout_secs`、`max_retries` 这类 Hub 专属调度参数不会从统一配置读取**，仍使用 `HubConfig::default()` 的默认值。

### 模式 2：legacy HubConfig

适用场景：

- 只想启动最小 Hub
- 需要显式控制 Hub 调度参数
- 暂时不需要 DingTalk / LLM

示例：

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

注意：legacy 模式**不包含** `[channels]` 或 `[llm]` 段，因此不能用于初始化 DingTalk 或 LLM。

---

## Hub 配置

### 方案 A：统一配置示例

这是当前最适合 Hub 生产运行的配置方式，尤其是需要 DingTalk Stream 或 LLM 时。

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
path = "/health"
verbose = false

[database]
path = "./data/uhorse.db"
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
ansi = true
file = true
line = true
target = true

[observability]
service_name = "uhorse-hub"
tracing_enabled = true
metrics_enabled = true
otlp_endpoint = ""
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
enabled = false
provider = "openai"
api_key = ""
base_url = "https://api.openai.com/v1"
model = "gpt-3.5-turbo"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse, a multi-channel AI gateway."
```

### 方案 B：legacy HubConfig 示例

这是最小 Hub 启动配置：

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

### Hub CLI 参数

```bash
./target/release/uhorse-hub --help
```

当前关键参数：

- `--config`：配置文件路径，默认 `hub.toml`
- `--log-level`：日志级别，默认 `info`
- `--host`：仅命令行配置模式生效，默认 `0.0.0.0`
- `--port`：仅命令行配置模式生效，默认 `8765`
- `--hub-id`：Hub ID，默认 `default-hub`

### 生成默认 Hub 配置

```bash
./target/release/uhorse-hub init --output hub.toml
```

`init` 生成的是**统一配置文件**，不是 legacy `HubConfig`。

---

## Node 配置

`uhorse-node` 只读取 `NodeConfig`，不区分统一/legacy 模式。

### 最小 Node 配置

```toml
name = "local-node"
workspace_path = "."

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = ""
```

### 完整 Node 配置示例

```toml
node_id = ""
name = "developer-macbook"
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

### Node CLI 参数

```bash
./target/release/uhorse-node --help
```

当前关键参数：

- `--config`：配置文件路径，默认 `node.toml`
- `--log-level`：日志级别，默认 `info`
- `--hub-url`：默认 `ws://localhost:8765/ws`
- `--workspace`：默认 `.`
- `--name`：默认 `uHorse-Node`

### Node 子命令

```bash
./target/release/uhorse-node init --output node.toml
./target/release/uhorse-node check --workspace /path/to/workspace
```

---

## DingTalk Stream 配置

当前 `uhorse-hub` 启用 DingTalk 时，推荐且默认文档路径是 **Stream 模式**。

### 最小 DingTalk 配置

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789
```

### 说明

- 当前主路径是 **Stream 模式**，不依赖公网 webhook 才能接收消息。
- Hub 仍保留 `GET/POST /api/v1/channels/dingtalk/webhook` 路由，用于兼容或辅助测试。
- 当前允许从 DingTalk 触发的管理命令是白名单：
  - `list` / `ls`
  - `search`
  - `read` / `cat`
  - `info`
  - `exists`

### 启用后会发生什么

当 `channels.enabled` 包含 `dingtalk` 时，Hub 启动阶段会：

1. 初始化 `DingTalkChannel`
2. 订阅 DingTalk 入站消息流
3. 把入站文本解析为 Hub 任务
4. 在任务完成后按原会话回发结果

---

## LLM 配置

当前 Hub 使用统一配置中的 `[llm]` 段初始化 `OpenAIClient`。

### 示例

```toml
[llm]
enabled = true
provider = "openai"
api_key = "sk-..."
base_url = "https://api.openai.com/v1"
model = "gpt-4.1"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

### 字段说明

| 字段 | 说明 |
|------|------|
| `enabled` | 是否启用 LLM |
| `provider` | 当前客户端的服务商标识 |
| `api_key` | API 密钥 |
| `base_url` | API 基础地址 |
| `model` | 模型名 |
| `temperature` | 采样温度 |
| `max_tokens` | 最大输出 token |
| `system_prompt` | 系统提示词 |

如果 `enabled = false`，Hub 启动时会跳过 LLM 初始化。

---

## 验证命令

### 生成默认配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 检查 Node 工作空间

```bash
./target/release/uhorse-node check --workspace .
```

### 启动 Hub 和 Node

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
./target/release/uhorse-node --config node.toml --log-level info
```

### 健康检查

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/api/nodes
```

注意：虽然统一配置里存在 `server.health.path` 字段，但当前 `uhorse-hub` Web 路由实际暴露的健康检查路径是：

```text
/api/health
```

---

## 建议

- 要跑 DingTalk 或 LLM：优先使用**统一配置**。
- 要跑最小本地闭环：优先使用 **legacy HubConfig + NodeConfig**。
- 如果你想同时做 Hub 调度参数微调和 DingTalk / LLM 初始化，当前需要了解这两种模式的边界，不要假设统一配置已经覆盖所有 Hub 专属字段。

更多示例见：

- [README.md](README.md)
- [LOCAL_SETUP.md](LOCAL_SETUP.md)
- [CHANNELS.md](CHANNELS.md)
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)
