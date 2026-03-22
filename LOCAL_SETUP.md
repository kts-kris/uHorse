# uHorse 本地开发与启动指南

本文档只描述 **当前仓库与代码实现对齐的 Hub-Node 本地路径**，重点覆盖：

- 本机启动 Hub
- 本机启动 Node
- Node 连接 Hub 并完成注册
- JWT 引导与审批接口
- Hub → Node → Hub 本地 roundtrip
- Hub 重启后的 Node 自动重连验证

## 目录

- [前置要求](#前置要求)
- [编译二进制](#编译二进制)
- [方式一：最小本地闭环](#方式一最小本地闭环)
- [方式二：带鉴权的主线回归](#方式二带鉴权的主线回归)
- [启动 Hub 和 Node](#启动-hub-和-node)
- [验证连接与任务链路](#验证连接与任务链路)
- [验证审批闭环](#验证审批闭环)
- [验证 Hub 重启后的 Node 重连](#验证-hub-重启后的-node-重连)
- [运行真实 roundtrip 集成测试](#运行真实-roundtrip-集成测试)
- [常见问题](#常见问题)

---

## 前置要求

- Rust `1.78+`
- 本地空闲端口，默认 `8765`
- 一个可写工作目录
- 如果工作目录不是 git 仓库，请把 `node.toml` 中 `require_git_repo = false`

如需启用 DingTalk 或 LLM，还需要：

- DingTalk 企业应用配置
- LLM API Key
- 若使用自定义模型服务商，需要兼容 OpenAI 风格的 `POST {base_url}/chat/completions`

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

这是最快的本地验证路径，适合先确认基础 Hub ↔ Node 链路。

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
require_git_repo = false
git_protection_enabled = true
watch_workspace = true
auto_git_add_new_files = true
internal_work_dir = ".uhorse"

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
```

这个组合适合先验证：

- Hub 启动
- Node 启动
- Node 连接 Hub
- 文件命令 roundtrip

---

## 方式二：带鉴权的主线回归

如果你要验证 **当前主线的 JWT Node 引导、审批接口与重连行为**，请让 Hub 使用统一配置并启用 `[security].jwt_secret`。

### 最小统一 `hub.toml`

```toml
[server]
host = "127.0.0.1"
port = 8765

[security]
jwt_secret = "replace-with-random-secret"
token_expiry = 86400
refresh_token_expiry = 2592000
pairing_expiry = 300
approval_enabled = true
pairing_enabled = true
```

> 注意：统一配置当前会驱动 Hub 的监听地址、DingTalk 初始化和 LLM 初始化；但 `max_nodes`、`heartbeat_timeout_secs`、`task_timeout_secs`、`max_retries` 这类 Hub 专属字段仍走 `HubConfig::default()`。

### 先为 Node 签发 token

启动 Hub 后执行：

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

当前实现中：

- 只有在 Hub 启用了 `SecurityManager` 时，该接口才可用，否则返回 `503`
- `credentials` 当前只要求是 **非空字符串**
- Node 注册时会校验 token 内的 `node_id` 必须和注册使用的 `node_id` 一致

### 带 token 的 `node.toml`

```toml
node_id = "office-node-01"
name = "office-node-01"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = "<access_token>"
```

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

### 检查工作区

```bash
./target/release/uhorse-node check --workspace .
```

---

## 验证连接与任务链路

### 1. 检查 Hub 健康状态

```bash
curl http://127.0.0.1:8765/api/health
```

当前实际对外健康检查路由是：

```text
/api/health
```

不是旧文档里的 `/health/live` 或 `/health/ready`。

### 2. 检查在线节点

```bash
curl http://127.0.0.1:8765/api/nodes
```

### 3. 提交一个最小文件任务

```bash
curl -X POST http://127.0.0.1:8765/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "command": {
      "type": "file",
      "action": "exists",
      "path": "/tmp/demo.txt"
    },
    "user_id": "api-user",
    "session_id": "api-session",
    "channel": "api"
  }'
```

### 4. 查询任务状态

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

说明：

- 当前 `GET /api/tasks` 仍是占位实现，会返回空列表
- 如果你要看真实状态，请直接使用 `GET /api/tasks/:task_id`

---

## 验证审批闭环

### 1. 给 Node 下发需要审批的规则

```bash
curl -X POST http://127.0.0.1:8765/api/nodes/office-node-01/permissions \
  -H "Content-Type: application/json" \
  -d '{
    "rules": [
      {
        "id": "approval-shell",
        "name": "Require shell approval",
        "resource": {
          "type": "command_type",
          "types": ["shell"]
        },
        "actions": ["execute"],
        "require_approval": true,
        "priority": 100,
        "enabled": true
      }
    ]
  }'
```

### 2. 提交一个 shell 任务

```bash
curl -X POST http://127.0.0.1:8765/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "command": {
      "type": "shell",
      "command": "pwd",
      "args": [],
      "cwd": null,
      "env": {},
      "timeout": 60,
      "capture_stderr": true
    },
    "user_id": "api-user",
    "session_id": "api-session",
    "channel": "api"
  }'
```

### 3. 查看待审批项

```bash
curl http://127.0.0.1:8765/api/approvals
```

### 4. 批准或拒绝

批准：

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/approve \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "approved"
  }'
```

拒绝：

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/reject \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "denied"
  }'
```

当前闭环是：

1. Node 因权限规则触发 `ApprovalRequest`
2. Hub 通过 `/api/approvals` 暴露请求
3. `/approve` 或 `/reject` 会向对应 Node 下发 `HubToNode::ApprovalResponse`
4. Node 继续执行或终止任务
5. 最终结果再经 `TaskResult` 回到 Hub

---

## 验证 Hub 重启后的 Node 重连

Node 连接循环内置自动重连逻辑，关键配置是：

- `connection.reconnect_interval_secs`
- `connection.max_reconnect_attempts`

建议验证顺序：

1. 保持 Node 进程存活
2. 重启 Hub
3. 再次请求 `GET /api/nodes`
4. 确认原节点重新出现
5. 再提交一个任务，确认重连后的任务链路恢复正常

---

## 运行真实 roundtrip 集成测试

当前最直接的本地闭环验证测试：

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

当前已覆盖的另一条关键测试：

```bash
cargo test -p uhorse-hub test_local_hub_rejects_node_with_mismatched_auth_token -- --nocapture
```

这两条测试分别覆盖：

- 真实 Hub + WebSocket + Node + 文件任务回传
- token 中 `node_id` 与注册 `node_id` 不一致时拒绝接入

---

## 常见问题

### `hub_url` 写错

Node 连接地址需要使用 `ws://` 或 `wss://`，并带 `/ws`：

```toml
hub_url = "ws://127.0.0.1:8765/ws"
```

### `/api/node-auth/token` 返回 `503`

说明当前 Hub 没有启用 `SecurityManager`。请确认：

- 使用的是统一配置启动 Hub
- `[security].jwt_secret` 已设置

### Node 启动时报工作区不是 git 仓库

默认 `require_git_repo = true`。如果你只是做本地验证，可显式设置：

```toml
require_git_repo = false
```

### `GET /api/tasks` 看不到任务

这是当前实现边界，不是运行异常。请使用：

```text
GET /api/tasks/:task_id
```

查看真实状态。
