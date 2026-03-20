# uHorse 本地开发与启动指南

本文档只描述 **当前仓库已对齐代码实现的本地 Hub-Node 路径**。

如果你的目标是：

- 在本机启动 Hub
- 在本机启动 Node
- 让 Node 连上 Hub
- 验证 Hub → Node → Hub 的本地 roundtrip
- 为后续 DingTalk Stream / LLM 配置做准备

请按本文档操作。

## 目录

- [前置要求](#前置要求)
- [编译二进制](#编译二进制)
- [方式一：最小本地闭环](#方式一本地最小闭环)
- [方式二：统一配置运行 Hub](#方式二统一配置运行-hub)
- [启动 Hub 和 Node](#启动-hub-和-node)
- [验证连接](#验证连接)
- [运行真实 roundtrip 集成测试](#运行真实-roundtrip-集成测试)
- [常见问题](#常见问题)
- [下一步](#下一步)

---

## 前置要求

- Rust `1.78+`
- 可用的本地工作目录
- 本地空闲端口（默认 `8765`）

如需启用 DingTalk 或 LLM：

- DingTalk 企业应用配置
- LLM API Key
- 若使用自定义模型服务商，需要一个 **OpenAI 兼容** 的 `/chat/completions` 端点

---

## 编译二进制

```bash
cargo build --release -p uhorse-hub -p uhorse-node
```

产物：

- `target/release/uhorse-hub`
- `target/release/uhorse-node`

---

## 方式一：最小本地闭环

这是当前最小、最容易验证的本地路径。

### `hub.toml`

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

### `node.toml`

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

这个组合适合先验证：

- Hub 启动
- Node 启动
- Node 连接 Hub
- 文件命令 roundtrip

---

## 方式二：统一配置运行 Hub

如果你要启用：

- DingTalk Stream
- LLM
- 自定义模型服务商

请让 Hub 使用统一配置文件。

### 示例 `hub.toml`

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
enabled = true
provider = "custom-provider"
api_key = "your_api_key"
base_url = "https://api.example.com/v1"
model = "your-model-name"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

### 关于自定义模型服务商

当前代码允许把 `provider` 配成任意未内置识别的字符串，例如：

- `provider = "custom-provider"`
- `provider = "my-company-llm"`
- `provider = "openai-compatible"`

这类值会被当作 **Custom provider** 处理。当前客户端会：

- 使用 `Bearer <api_key>` 鉴权
- 请求 `{base_url}/chat/completions`
- 发送 OpenAI 兼容的 `messages` / `temperature` / `max_tokens` 结构

所以你的自定义服务商需要兼容这一路径和请求格式。

> 注意：统一配置当前会驱动 Hub 的监听地址、DingTalk 初始化和 LLM 初始化；但 `max_nodes`、`heartbeat_timeout_secs`、`task_timeout_secs`、`max_retries` 这类 Hub 专属字段仍不从统一配置读取。

---

## 启动 Hub 和 Node

### 启动 Hub

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

### 启动 Node

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

### 检查 Node 工作空间

```bash
./target/release/uhorse-node check --workspace .
```

---

## 验证连接

### 1. 检查 Hub 健康状态

```bash
curl http://127.0.0.1:8765/api/health
```

### 2. 检查在线节点

```bash
curl http://127.0.0.1:8765/api/nodes
```

如果 `/api/nodes` 返回在线节点列表，说明 Node 已连上 Hub。

### 3. 查看日志重点

Hub 启动时可关注：

- 配置是否成功加载
- DingTalk 是否初始化
- LLM 是否初始化
- `/ws` 是否有 Node 连接
- 任务完成后是否出现 DingTalk 结果回发日志

Node 启动时可关注：

- 工作空间是否校验通过
- 是否成功连接 `hub_url`
- 是否持续发送心跳

---

## 运行真实 roundtrip 集成测试

当前仓库已经有一条真实本地闭环测试：

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

这条测试会真实启动：

1. 一个 Hub
2. 一个 WebSocket 服务
3. 一个 Node
4. 一个文件存在性命令
5. Node 执行结果回传到 Hub

这是当前最直接的本地闭环验证方式。

如果你还要继续验证 DingTalk 最后一跳，可在统一配置里填入真实企业凭据后补做一轮真实消息联调。当前主线已经完成一次真实企业验证：非法命令会即时错误回显，合法 `exists` 命令会把 JSON 结果原路回传到原会话。

---

## 常见问题

### `hub_url` 写错

Node 连接 Hub 需要 `ws://` 或 `wss://`，并带 `/ws`：

```toml
hub_url = "ws://127.0.0.1:8765/ws"
```

### 健康检查路径不对

当前实际健康检查路由是：

```text
/api/health
```

不是旧文档里的 `/health/live` 或 `/health/ready`。

### 自定义模型服务商调用失败

优先检查：

- `provider` 是否只是自定义标识，不影响核心逻辑
- `base_url` 是否是 API 根路径
- 服务商是否兼容 `POST {base_url}/chat/completions`
- 是否接受 Bearer Token

---

## 下一步

- [CONFIG.md](CONFIG.md)：完整配置手册
- [CHANNELS.md](CHANNELS.md)：DingTalk Stream 说明
- [TESTING.md](TESTING.md)：测试与验证命令
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)：v4.0 部署路径
