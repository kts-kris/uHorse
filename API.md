# uHorse API 使用指南

本文档只描述 **当前仓库已经实现并用于 v4.0 Hub-Node 主线** 的 API。示例默认 Hub 地址为 `http://127.0.0.1:8765`。

## 目录

- [范围](#范围)
- [通用响应格式](#通用响应格式)
- [健康检查](#健康检查)
- [Node 接入与鉴权](#node-接入与鉴权)
- [任务 API](#任务-api)
- [审批 API](#审批-api)
- [DingTalk 兼容回调](#dingtalk-兼容回调)
- [手工联调顺序](#手工联调顺序)
- [相关文档](#相关文档)

---

## 范围

当前 Hub 运行时实际暴露并与 Hub-Node 主链相关的接口主要是：

- `GET /api/health`
- `GET /metrics`
- `GET /ws`
- `GET /api/stats`
- `GET /api/nodes`
- `GET /api/nodes/:node_id`
- `POST /api/nodes/:node_id/permissions`
- `GET /api/tasks`
- `POST /api/tasks`
- `GET /api/tasks/:task_id`
- `POST /api/tasks/:task_id/cancel`
- `GET /api/approvals`
- `GET /api/approvals/:request_id`
- `POST /api/approvals/:request_id/approve`
- `POST /api/approvals/:request_id/reject`
- `POST /api/node-auth/token`
- `GET/POST /api/v1/channels/dingtalk/webhook`

> 注意：本文档不再把旧版 `/health/live`、`/health/ready`、`/api/v1/auth/*`、`/api/v1/messages` 当作当前主线 API。

---

## 通用响应格式

除 `GET /api/health` 与 `GET /metrics` 外，当前 Hub Web API 统一使用如下包装结构：

```json
{
  "success": true,
  "data": {},
  "error": null
}
```

失败时：

```json
{
  "success": false,
  "data": null,
  "error": "error message"
}
```

### `GET /api/health` 的特殊返回

健康检查接口直接返回 JSON，不包在 `ApiResponse<T>` 中：

```json
{
  "status": "healthy",
  "version": "4.0.0-alpha.3"
}
```

---

## 健康检查

### `GET /api/health`

用于确认 Hub HTTP 服务已经启动。

```bash
curl http://127.0.0.1:8765/api/health
```

成功响应：

```json
{
  "status": "healthy",
  "version": "4.0.0-alpha.3"
}
```

---

## 指标接口

### `GET /metrics`

用于抓取当前 Hub 暴露的 Prometheus 指标文本。

```bash
curl http://127.0.0.1:8765/metrics
```

返回内容类型：

```text
text/plain; version=0.0.4; charset=utf-8
```

当前输出会同时包含 exporter 指标和 Hub 统计指标。

---

## Node 接入与鉴权

### 1. 签发 Node JWT：`POST /api/node-auth/token`

当 Hub 使用 **统一配置** 启动，且 `[security].jwt_secret` 已配置时，可以通过该接口为 Node 签发 token。

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

成功响应：

```json
{
  "success": true,
  "data": {
    "node_id": "office-node-01",
    "access_token": "...",
    "refresh_token": "...",
    "expires_at": "2026-03-23T12:00:00+00:00"
  },
  "error": null
}
```

注意：

- 如果 Hub 没有启用 `SecurityManager`，该接口会返回 `503`。
- 当前实现里，`credentials` 只要求是**非空字符串**，适合本地引导和受控环境。
- Node 注册时会校验 `access_token` 里的 `node_id` 是否与注册的 `node_id` 一致。

### 2. Node WebSocket 连接：`GET /ws`

Node 通过 `node.toml` 中的 `connection.hub_url` 连接 WebSocket，例如：

```toml
[connection]
hub_url = "ws://127.0.0.1:8765/ws"
auth_token = "<access_token>"
```

当前主线已验证：

- Node 注册
- 心跳
- 任务下发
- 审批响应回传
- Hub 重启后的 Node 自动重连与重新注册

### 3. 查询节点：`GET /api/nodes`

```bash
curl http://127.0.0.1:8765/api/nodes
```

用于确认在线节点列表是否已经出现。

### 4. 查询单个节点：`GET /api/nodes/:node_id`

```bash
curl http://127.0.0.1:8765/api/nodes/office-node-01
```

### 5. 更新节点权限：`POST /api/nodes/:node_id/permissions`

该接口会把权限规则下发给在线节点。

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

---

## 任务 API

### 1. 提交任务：`POST /api/tasks`

请求体字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `command` | object | 当前支持 `file`、`shell` 等协议命令 |
| `user_id` | string | 用户标识 |
| `session_id` | string | 会话标识 |
| `channel` | string | 渠道，例如 `api`、`dingtalk` |
| `intent` | string? | 可选，业务意图 |
| `env` | object | 可选，环境变量 |
| `priority` | string | 可选，默认 `normal` |
| `workspace_hint` | string? | 可选，工作区匹配提示 |
| `required_tags` | string[] | 可选，节点标签过滤 |

文件存在性任务示例：

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
    "channel": "api",
    "priority": "high",
    "required_tags": []
  }'
```

成功响应：

```json
{
  "success": true,
  "data": {
    "task_id": "task-0"
  },
  "error": null
}
```

### 2. 查询任务状态：`GET /api/tasks/:task_id`

```bash
curl http://127.0.0.1:8765/api/tasks/task-0
```

成功响应示例：

```json
{
  "success": true,
  "data": {
    "task_id": "task-0",
    "status": "Running",
    "command_type": "file",
    "priority": "high",
    "started_at": "2026-03-23T12:00:00+00:00"
  },
  "error": null
}
```

说明：

- `status` 可能是 `Queued`、`Running`、`Completed`、`Failed`。
- 当前实现已经保证 `command_type` 与 `priority` 返回**真实任务元数据**，不再写死为默认值。
- `command_type` 来自调度器里的真实命令类型，例如 `file`、`shell`。

### 3. 取消任务：`POST /api/tasks/:task_id/cancel`

```bash
curl -X POST http://127.0.0.1:8765/api/tasks/task-0/cancel
```

### 4. 列出任务：`GET /api/tasks`

```bash
curl http://127.0.0.1:8765/api/tasks
```

注意：当前实现里的 `GET /api/tasks` 仍是**占位实现**，会返回空列表；如果要查询真实任务状态，请直接使用 `GET /api/tasks/:task_id`。

---

## 审批 API

以下接口要求 Hub 已启用 `SecurityManager`；否则会返回 `503`。

### 1. 列出待审批：`GET /api/approvals`

```bash
curl http://127.0.0.1:8765/api/approvals
```

返回值 `data` 是 `ApprovalRequest[]`。关键字段包括：

- `id`
- `action`
- `requested_by`
- `status`
- `created_at`
- `expires_at`
- `metadata`

### 2. 获取单个审批：`GET /api/approvals/:request_id`

```bash
curl http://127.0.0.1:8765/api/approvals/<request_id>
```

### 3. 批准审批：`POST /api/approvals/:request_id/approve`

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/approve \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "允许执行"
  }'
```

### 4. 拒绝审批：`POST /api/approvals/:request_id/reject`

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/reject \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "拒绝高风险命令"
  }'
```

当前主线已验证的闭环是：

1. Node 根据本地权限规则触发审批请求。
2. Hub 通过 `/api/approvals` 暴露待审批项。
3. 调用 `/approve` 或 `/reject` 后，Hub 会向对应 Node 下发 `ApprovalResponse`。
4. Node 收到审批结果后继续执行或终止任务。
5. 最终再通过 `TaskResult` 回传结果。

---

## DingTalk 兼容回调

当前仍保留以下兼容路由：

- `GET /api/v1/channels/dingtalk/webhook`
- `POST /api/v1/channels/dingtalk/webhook`

它们主要用于兼容或辅助测试；当前推荐叙事仍是 **DingTalk Stream 模式 + Hub 任务链路**。

---

## 手工联调顺序

推荐用下面这条顺序做一次完整回归：

1. 用统一配置启动 Hub，并确保 `[security].jwt_secret` 已设置。
2. 调用 `POST /api/node-auth/token` 给稳定 `node_id` 签发 token。
3. 把 `access_token` 写入 `node.toml` 的 `connection.auth_token`。
4. 启动 Node，检查 `GET /api/nodes`。
5. 调用 `POST /api/tasks` 提交文件任务，检查 `GET /api/tasks/:task_id`。
6. 配置 shell 命令需要审批，提交 shell 任务。
7. 通过 `GET /api/approvals` 找到待审批项，再调用 `/approve` 或 `/reject`。
8. 再次检查 `GET /api/tasks/:task_id`，确认状态进入 `Completed` 或 `Failed`。
9. 保持 Node 存活，重启 Hub，确认 Node 自动重连并重新出现在 `GET /api/nodes`。
10. 重连后再次提交任务，确认链路恢复正常。

---

## 相关文档

- [README.md](README.md)：项目总览
- [LOCAL_SETUP.md](LOCAL_SETUP.md)：本地双进程联调
- [CONFIG.md](CONFIG.md)：统一配置与 Node 配置
- [TESTING.md](TESTING.md)：测试与回归命令
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)：部署路径
- [docs/architecture/v4.0-architecture.md](docs/architecture/v4.0-architecture.md)：架构说明
