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
path = "/api/health"
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
ansi = true
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
git_protection_enabled = true
watch_workspace = true
auto_git_add_new_files = true
require_git_repo = true
internal_work_dir = ".uhorse"

[connection]
hub_url = "wss://hub.example.com/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = ""
```

### Node 工作区保护

当前 `uhorse-node` 默认会把执行限制在 `workspace_path` 内，并额外启用以下行为：

- `git_protection_enabled = true`：拒绝危险 git 命令
- `watch_workspace = true`：监听工作区新增文件
- `auto_git_add_new_files = true`：对新增文件执行本地 `git add`
- `require_git_repo = true`：要求工作区本身就是 git 仓库
- `internal_work_dir = ".uhorse"`：内部临时代码目录，默认不会被 watcher 自动加入 git
- 文件写入会自动创建工作区内缺失的父目录，但仍会拒绝任何越出 `workspace_path` 的父路径逃逸
- 浏览器命令会区分 `OpenSystem` 与 `Navigate` 语义；当前 DingTalk “打开网页”主链会落到 `OpenSystem`

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

[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"

[[channels.dingtalk.skill_installers]]
user_id = "your-admin-user-id"
# staff_id = "your-staff-id"
# corp_id = "dingcorp-xxx"
```

### 说明

- 当前主路径是 **Stream 模式**，不依赖公网 webhook 才能接收消息。
- Hub 仍保留 `GET/POST /api/v1/channels/dingtalk/webhook` 路由，用于兼容或辅助测试。
- 若要镜像 Node Desktop 本地通知到钉钉，当前主路径是启用 pairing，并在 Node Desktop 中发起绑定、在 DingTalk 中确认；`channels.dingtalk.notification_bindings` 仅用于兼容 seed/fallback。
- `[[channels.dingtalk.skill_installers]]` 只限制 DingTalk 文本安装入口，不限制 HTTP `POST /api/v1/skills/install`。
- 当前默认快速回归已包含 Agent Browser Skill 安装 smoke，可通过 `make skill-install-smoke` 单独执行。
- 白名单匹配支持 `user_id` / `staff_id`，并可选叠加 `corp_id` 限制企业范围。
- DingTalk 文本会先经过 LLM 规划，再转换为单个安全命令；文件操作、shell，以及受控 `BrowserCommand` 都在当前主线上。
- 但 `安装技能 <package> <download_url> [version]` / `install skill ...` 会直接走 Skill 安装薄入口，不经过通用自然语言命令规划。
- 当前在线安装支持 `.zip` / `.tar.gz`，zip 包可带一层嵌套根目录；若是仅提供 `skill.yaml` 的 Python Skill，安装时会自动生成 `skill.toml`，并在存在 `requirements.txt` 时创建 `.venv` 安装依赖。
- 对于“打开网页”这类场景，当前主链会优先规划为 `BrowserCommand::OpenSystem`，而不是自动化浏览器 `Navigate`。
- Hub 会在本地下发前校验路径范围，并拒绝危险 git 命令。

### 启用后会发生什么

当 `channels.enabled` 包含 `dingtalk` 时，Hub 启动阶段会：

1. 初始化 `DingTalkChannel`
2. 订阅 DingTalk 入站消息流
3. 把入站自然语言交给 LLM 规划为安全命令
4. 把通过本地校验的命令提交为 Hub 任务
5. 在任务完成后优先通过 LLM 总结结果，再经 `session_webhook` 原路回发；当 webhook 不可用或已过期时，回退到群消息或单聊发送

当前主线已经完成一次真实企业租户验证：

- 非法或不安全请求会即时错误回显
- 合法请求的执行结果会原路回传到原会话

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

注意：当前 `uhorse-hub` 仅在 `server.health.enabled = true` 时暴露统一配置里的 `server.health.path`；如果未显式配置，则默认使用：

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
