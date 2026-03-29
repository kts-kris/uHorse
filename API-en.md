# uHorse API Reference

This document only covers the APIs that are **actually implemented and used by the current v4.1.2 Hub-Node mainline**. Examples assume the Hub is reachable at `http://127.0.0.1:8765`.

## Table of Contents

- [Scope](#scope)
- [Common Response Format](#common-response-format)
- [Health Check](#health-check)
- [Node Access and Authentication](#node-access-and-authentication)
- [Task APIs](#task-apis)
- [Runtime Session APIs](#runtime-session-apis)
- [Approval APIs](#approval-apis)
- [DingTalk Compatibility Webhook](#dingtalk-compatibility-webhook)
- [Manual Regression Order](#manual-regression-order)
- [Related Documents](#related-documents)

---

## Scope

The current Hub runtime exposes these Hub-Node related endpoints:

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
- `GET /api/v1/sessions`
- `GET /api/v1/sessions/:session_id`
- `GET /api/v1/sessions/:session_id/messages`
- `GET /api/approvals`
- `GET /api/approvals/:request_id`
- `POST /api/approvals/:request_id/approve`
- `POST /api/approvals/:request_id/reject`
- `POST /api/node-auth/token`
- `GET/POST /api/v1/channels/dingtalk/webhook`

> Note: this document no longer treats the old `/health/live`, `/health/ready`, `/api/v1/auth/*`, or `/api/v1/messages` endpoints as the mainline API surface.

---

## Common Response Format

Except for `GET /api/health` and `GET /metrics`, current Hub HTTP APIs use the same wrapper shape:

```json
{
  "success": true,
  "data": {},
  "error": null
}
```

Failure case:

```json
{
  "success": false,
  "data": null,
  "error": "error message"
}
```

### Special case: `GET /api/health`

The health endpoint returns plain JSON instead of `ApiResponse<T>`:

```json
{
  "status": "healthy",
  "version": "4.1.2"
}
```

---

## Health Check

### `GET /api/health`

Use this to confirm the Hub HTTP service is up.

```bash
curl http://127.0.0.1:8765/api/health
```

Successful response:

```json
{
  "status": "healthy",
  "version": "4.1.2"
}
```

---

## Metrics Endpoint

### `GET /metrics`

Use this to scrape the Prometheus metrics exposed by the current Hub.

```bash
curl http://127.0.0.1:8765/metrics
```

Response content type:

```text
text/plain; version=0.0.4; charset=utf-8
```

The output currently includes both exporter metrics and Hub stats.

---

## Node Access and Authentication

### 1. Issue a Node JWT: `POST /api/node-auth/token`

When the Hub starts from a **unified config** and `[security].jwt_secret` is configured, this endpoint can issue a token pair for a Node.

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

Successful response:

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

Notes:

- If the Hub is running without `SecurityManager`, the endpoint returns `503`.
- In the current implementation, `credentials` only needs to be a **non-empty string**, which is suitable for controlled bootstrap flows.
- During registration, the Hub validates that the token `node_id` matches the registering `node_id`.

### 2. Node WebSocket connection: `GET /ws`

Nodes connect through the `connection.hub_url` in `node.toml`, for example:

```toml
[connection]
hub_url = "ws://127.0.0.1:8765/ws"
auth_token = "<access_token>"
```

The current mainline has already been verified for:

- Node registration
- heartbeats
- task dispatch
- approval response delivery
- automatic Node reconnect and re-registration after Hub restart

### 3. List nodes: `GET /api/nodes`

```bash
curl http://127.0.0.1:8765/api/nodes
```

Use this to confirm that an online node is visible to the Hub.

The response includes `workspace.workspace_id`, which is the stable execution workspace identifier bound by the Hub. If the Node does not send one during registration, the Hub backfills it as `exec:{node_id}:{workspace_name}`.

Example response fragment:

```json
{
  "success": true,
  "data": [
    {
      "node_id": "office-node-01",
      "name": "Office Node",
      "workspace": {
        "workspace_id": "exec:office-node-01:workspace",
        "name": "workspace",
        "path": "/Users/demo/workspace",
        "read_only": false,
        "allowed_patterns": ["**/*"],
        "denied_patterns": []
      }
    }
  ]
}
```

### 4. Get one node: `GET /api/nodes/:node_id`

```bash
curl http://127.0.0.1:8765/api/nodes/office-node-01
```

The single-node response also returns `workspace.workspace_id`, which can be used for later task routing.

### 5. Push permission rules to a node: `POST /api/nodes/:node_id/permissions`

This endpoint sends permission rules to an online node.

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

## Task APIs

### 1. Submit a task: `POST /api/tasks`

Request fields:

| Field | Type | Description |
|-------|------|-------------|
| `command` | object | protocol command, currently including `file`, `shell`, and controlled `browser` commands |
| `user_id` | string | user identifier |
| `session_id` | string | session identifier |
| `channel` | string | source channel such as `api` or `dingtalk` |
| `execution_workspace_id` | string? | optional target execution workspace identifier; when present, routing prefers this bound Node |
| `collaboration_workspace_id` | string? | optional logical collaboration workspace identifier; currently forwarded as task context only |
| `intent` | string? | optional business intent |
| `env` | object | optional environment variables |
| `priority` | string | optional, defaults to `normal` |
| `workspace_hint` | string? | optional workspace match hint; used as a compatibility fallback only when `execution_workspace_id` is absent |
| `required_tags` | string[] | optional node tag filter |

Example file-exists task:

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
    "execution_workspace_id": "exec:office-node-01:workspace",
    "collaboration_workspace_id": "collab:team-a:session-1",
    "priority": "high",
    "required_tags": []
  }'
```

Routing precedence:

1. When `execution_workspace_id` is provided, the Hub routes by that stable identifier first.
2. When `execution_workspace_id` is absent, the Hub falls back to `workspace_hint` path matching.
3. `collaboration_workspace_id` is currently forwarded as logical collaboration context only and does not change the real execution directory.

Successful response:

```json
{
  "success": true,
  "data": {
    "task_id": "task-0"
  },
  "error": null
}
```

Browser task notes:

- the current mainline plans DingTalk “open a webpage” requests as `BrowserCommand::OpenSystem`
- Hub validates target URLs first and rejects `file://`, localhost, and private-network addresses
- when a file write succeeds, Node returns structured `file_operation` JSON and the Hub summary text prefers `Saved successfully: <path>` style output

### 2. Get task status: `GET /api/tasks/:task_id`

```bash
curl http://127.0.0.1:8765/api/tasks/task-0
```

Example response:

```json
{
  "success": true,
  "data": {
    "task_id": "task-0",
    "status": "Running",
    "command_type": "file",
    "priority": "high",
    "execution_workspace_id": "exec:office-node-01:workspace",
    "collaboration_workspace": {
      "collaboration_workspace_id": "collab:web-session",
      "scope_owner": "session:api:api-user",
      "members": [],
      "default_agent_id": null,
      "visible_scope_chain": [
        "user:api:api-user",
        "role:web-org:manager",
        "enterprise:web-org",
        "global"
      ],
      "bound_execution_workspace_id": "exec:office-node-01:workspace",
      "materialization": "none"
    },
    "started_at": "2026-03-23T12:00:00+00:00"
  },
  "error": null
}
```

Notes:

- `status` can be `Queued`, `Running`, `Completed`, or `Failed`.
- The current implementation now returns the **real** `command_type` and `priority` from scheduler state instead of hard-coded defaults.
- `command_type` comes from the actual queued/running/completed task metadata, such as `file` or `shell`.
- `execution_workspace_id` is the real execution workspace identity, while `collaboration_workspace` is the Hub-side logical collaboration view and not the Node's actual directory.
- `collaboration_workspace.materialization` is currently fixed to `none`, which means v4.1.2 only keeps a logical collaboration layer and does not materialize real directories on the Hub side.

### 3. Cancel a task: `POST /api/tasks/:task_id/cancel`

```bash
curl -X POST http://127.0.0.1:8765/api/tasks/task-0/cancel
```

### 4. List tasks: `GET /api/tasks`

```bash
curl http://127.0.0.1:8765/api/tasks
```

Note: the current `GET /api/tasks` implementation is still a **placeholder** and returns an empty list. Use `GET /api/tasks/:task_id` when you need real task status.

---

## Runtime Session APIs

The current runtime exposes these read-only endpoints:

- `GET /api/v1/sessions`
- `GET /api/v1/sessions/:session_id`
- `GET /api/v1/sessions/:session_id/messages`

### 1. List runtime sessions: `GET /api/v1/sessions`

```bash
curl http://127.0.0.1:8765/api/v1/sessions
```

The `data` field is `SessionRuntimeSummary[]`. Key fields include:

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | runtime session identifier |
| `agent_id` | string? | currently selected default Agent |
| `conversation_id` | string? | upstream conversation identifier |
| `sender_user_id` / `sender_staff_id` | string? | channel-side user identity |
| `last_task_id` | string? | most recent task ID |
| `message_count` | number | accumulated runtime-side message count |
| `namespace` | object? | restored `SessionNamespace`, including `global / tenant / enterprise / department / roles / user / session` |
| `collaboration_workspace` | object? | Hub-side logical collaboration workspace view |

### 2. Get one session detail: `GET /api/v1/sessions/:session_id`

```bash
curl http://127.0.0.1:8765/api/v1/sessions/dingtalk:user-api
```

Example response:

```json
{
  "success": true,
  "data": {
    "session_id": "dingtalk:user-api",
    "agent_id": "main",
    "conversation_id": "conv-api",
    "sender_user_id": "user-api",
    "sender_staff_id": "staff-api",
    "last_task_id": null,
    "message_count": 1,
    "created_at": "2026-03-29T08:00:00+00:00",
    "last_active": "2026-03-29T08:01:00+00:00",
    "namespace": {
      "global": "global",
      "tenant": null,
      "enterprise": null,
      "department": null,
      "roles": [],
      "user": "user:dingtalk:user-api",
      "session": "session:dingtalk:user-api"
    },
    "collaboration_workspace": {
      "collaboration_workspace_id": "collab:session:dingtalk:user-api",
      "scope_owner": "session:dingtalk:user-api",
      "members": ["user:dingtalk:user-api"],
      "default_agent_id": "main",
      "visible_scope_chain": ["user:dingtalk:user-api", "global"],
      "bound_execution_workspace_id": null,
      "materialization": "none"
    },
    "memory_context_chain": [
      "global",
      "user:dingtalk:user-api",
      "session:dingtalk:user-api"
    ],
    "visibility_chain": [
      "user:dingtalk:user-api",
      "global"
    ],
    "metadata": {
      "namespace_global": "global",
      "namespace_user": "user:dingtalk:user-api",
      "namespace_session": "session:dingtalk:user-api",
      "collaboration_workspace_id": "collab:session:dingtalk:user-api"
    }
  },
  "error": null
}
```

Notes:

- `memory_context_chain` is the memory read order from shared scopes to private scopes.
- `visibility_chain` is the Agent / Skill resolution order from private scopes back to shared scopes.
- `metadata` may include runtime restoration fields such as `namespace_tenant`, `namespace_enterprise`, `namespace_department`, `namespace_roles`, `execution_workspace_id`, `agent_source_layer`, and `agent_source_scope`.
- `collaboration_workspace` is a Hub-side logical entity that only carries collaboration scope, default Agent binding, and execution workspace binding. It does not directly change the Node's real working directory.

### 3. Read session message history: `GET /api/v1/sessions/:session_id/messages`

```bash
curl http://127.0.0.1:8765/api/v1/sessions/dingtalk:user-api/messages
```

Example response:

```json
{
  "success": true,
  "data": [
    {
      "timestamp": "2026-03-29 08:01:00 UTC",
      "user_message": "hello",
      "assistant_message": "world"
    }
  ],
  "error": null
}
```

---

## Approval APIs

These endpoints require the Hub to run with `SecurityManager`; otherwise they return `503`.

### 1. List pending approvals: `GET /api/approvals`

```bash
curl http://127.0.0.1:8765/api/approvals
```

The `data` field is `ApprovalRequest[]`. Key fields include:

- `id`
- `action`
- `requested_by`
- `status`
- `created_at`
- `expires_at`
- `metadata`

### 2. Get one approval: `GET /api/approvals/:request_id`

```bash
curl http://127.0.0.1:8765/api/approvals/<request_id>
```

### 3. Approve: `POST /api/approvals/:request_id/approve`

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/approve \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "approved"
  }'
```

### 4. Reject: `POST /api/approvals/:request_id/reject`

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/reject \
  -H "Content-Type: application/json" \
  -d '{
    "responder": "admin",
    "reason": "high-risk command denied"
  }'
```

The verified approval loop in the current mainline is:

1. The Node triggers an approval request from its local permission rules.
2. The Hub exposes it through `/api/approvals`.
3. Calling `/approve` or `/reject` makes the Hub send `ApprovalResponse` to the corresponding Node.
4. The Node resumes or aborts task execution.
5. The final task result is then returned through `TaskResult`.

---

## DingTalk Compatibility Webhook

The following compatibility routes still exist:

- `GET /api/v1/channels/dingtalk/webhook`
- `POST /api/v1/channels/dingtalk/webhook`

They are mainly kept for compatibility or auxiliary testing. The recommended mainline path is still **DingTalk Stream mode + Hub task pipeline**.

---

## Manual Regression Order

Use this order for a full end-to-end regression:

1. Start the Hub from unified config and make sure `[security].jwt_secret` is set.
2. Call `POST /api/node-auth/token` to issue a token for a stable `node_id`.
3. Put the returned `access_token` into `node.toml` as `connection.auth_token`.
4. Start the Node and check `GET /api/nodes`.
5. Call `POST /api/tasks` for a file task and inspect `GET /api/tasks/:task_id`.
6. Configure shell execution to require approval, then submit a shell task.
7. Use `GET /api/approvals` to find the pending request, then call `/approve` or `/reject`.
8. Check `GET /api/tasks/:task_id` again and confirm it reaches `Completed` or `Failed`.
9. Keep the Node alive, restart the Hub, and confirm the Node reconnects and appears again in `GET /api/nodes`.
10. Submit another task after reconnect and confirm the pipeline is healthy again.

---

## Related Documents

- [README-en.md](README-en.md): project overview
- [LOCAL_SETUP.md](LOCAL_SETUP.md): local dual-process setup
- [CONFIG-en.md](CONFIG-en.md): unified config and Node config
- [TESTING.md](TESTING.md): test and regression commands
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md): deployment path
- [docs/architecture/v4.0-architecture-en.md](docs/architecture/v4.0-architecture-en.md): architecture details
