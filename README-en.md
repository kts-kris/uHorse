<p align="center">
  <strong>English</strong> | <a href="README.md">简体中文</a>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>v4.0 Hub-Node Distributed AI Execution Platform</strong>
</p>

<p align="center">
  <em>Hub handles scheduling and channel intake, while Node executes locally and returns results.</em>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#current-status">Current Status</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#documentation-index">Docs</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.0.0--alpha.3-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-yellow" alt="Status">
</p>

---

## Overview

The current mainline is the **v4.0 Hub-Node architecture**:

- `uhorse-hub`: cloud-side control plane for Node access, task scheduling, Web API, approval endpoints, and DingTalk Stream intake.
- `uhorse-node`: local execution node binary.
- `uhorse-node-runtime`: the real Node runtime implementation, including reconnect, workspace protection, permissions, approval requests, and task execution.
- `uhorse-protocol`: protocol types shared by Hub and Node, including `TaskAssignment`, `TaskResult`, `ApprovalRequest`, and `ApprovalResponse`.
- `uhorse-config`: unified Hub config model covering `server`, `channels`, `security`, `llm`, and related sections.

These docs are aligned to what is **actually implemented and exercised in the repository today**. They no longer treat `/health/live`, `/health/ready`, `/api/v1/auth/*`, or `/api/v1/messages` as the current mainline.

## Current Status

| Capability | Status | Notes |
|------------|--------|-------|
| Local Hub startup | ✅ | the current observability endpoints are `GET /api/health` and `GET /metrics` |
| Local Node startup | ✅ | `uhorse-node` loads `node.toml` and connects to `ws://.../ws` |
| Node JWT bootstrap | ✅ | `POST /api/node-auth/token` issues tokens when `[security].jwt_secret` is configured |
| Hub → Node dispatch | ✅ | `POST /api/tasks` submits work into the scheduler |
| Node → Hub result return | ✅ | Node sends full `NodeToHub::TaskResult` |
| Approval loop | ✅ | `ApprovalRequest -> /api/approvals -> ApprovalResponse -> TaskResult` |
| Node reconnect after Hub restart | ✅ | Node reconnects and re-registers automatically |
| Real local integration test | ✅ | `test_local_hub_node_roundtrip_file_exists` covers a real Hub + Node + WebSocket roundtrip |
| Auth rejection path | ✅ | `test_local_hub_rejects_node_with_mismatched_auth_token` covers token / registration `node_id` mismatch |
| DingTalk Stream integration | ✅ | Stream mode is the recommended path; mirroring Node Desktop notifications also requires `channels.dingtalk.notification_bindings` |

## Quick Start

### 1. Build the binaries

```bash
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs
cargo build --release -p uhorse-hub -p uhorse-node
```

Build outputs:

- `target/release/uhorse-hub`
- `target/release/uhorse-node`

### 2. Generate default configs

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 3. Smallest local roundtrip

If you only want the smallest Hub ↔ Node loop first, use a minimal config.

`hub.toml`:

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

`node.toml`:

```toml
name = "local-node"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
```

Start both processes:

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
./target/release/uhorse-node --config node.toml --log-level info
```

Verify:

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 4. Enable auth and approvals

To match the current authenticated Hub-Node mainline, run Hub from unified config and set `[security].jwt_secret`:

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

Then issue a token for a stable `node_id`:

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

Put the returned `access_token` into `node.toml`:

```toml
name = "office-node-01"
node_id = "office-node-01"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
auth_token = "<access_token>"
```

If you want to mirror Node Desktop local notifications to DingTalk, both sides must be configured:

- enable `mirror_notifications_to_dingtalk` in the local Node Desktop settings
- add a `node_id` → DingTalk `user_id` mapping in `channels.dingtalk.notification_bindings` on Hub

### 5. Submit a minimal task

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

Query real task status with:

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

> Note: `GET /api/tasks` is still a placeholder in the current implementation. Use `GET /api/tasks/:task_id` for real status.

## Architecture

```text
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  • Web API: /api/health /metrics /api/*     │
│  • WebSocket: /ws                            │
│  • Task Scheduler                            │
│  • Approval API                              │
│  • DingTalk Stream                           │
└──────────────────────────────────────────────┘
                      │
                      │ WebSocket
                      ▼
┌──────────────────────────────────────────────┐
│             uhorse-node-runtime              │
│  • Connection loop / reconnect               │
│  • Workspace protection                      │
│  • Permission manager                        │
│  • Command executor                          │
│  • TaskResult / ApprovalRequest              │
└──────────────────────────────────────────────┘
                      │
                      ▼
               ┌───────────────┐
               │   workspace   │
               └───────────────┘
```

### Primary source entrypoints

- Hub startup and unified config loading: `crates/uhorse-hub/src/main.rs`
- Hub Web API: `crates/uhorse-hub/src/web/mod.rs`
- Hub WebSocket auth and registration: `crates/uhorse-hub/src/web/ws.rs`
- Hub scheduler: `crates/uhorse-hub/src/task_scheduler.rs`
- Node CLI entrypoint: `crates/uhorse-node/src/main.rs`
- Node runtime: `crates/uhorse-node-runtime/src/node.rs`
- Node connection loop: `crates/uhorse-node-runtime/src/connection.rs`
- Local integration tests: `crates/uhorse-hub/tests/integration_test.rs`

## Documentation Index

| Document | Description |
|----------|-------------|
| [API-en.md](API-en.md) | current implemented Hub-Node API surface |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | local dual-process setup, JWT bootstrap, approval, and reconnect regression |
| [CONFIG-en.md](CONFIG-en.md) | unified config, legacy HubConfig, NodeConfig, and permission rules |
| [CHANNELS-en.md](CHANNELS-en.md) | current channel status, focused on DingTalk Stream |
| [TESTING.md](TESTING.md) | package tests, workspace tests, and manual regression order |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4.0 Hub-Node deployment guide |
| [docs/architecture/v4.0-architecture-en.md](docs/architecture/v4.0-architecture-en.md) | v4.0 architecture details |

## Workspace Layout

```text
crates/
├── uhorse-hub/           # cloud hub
├── uhorse-node/          # Node CLI binary entrypoint
├── uhorse-node-runtime/  # actual Node runtime
├── uhorse-protocol/      # Hub-Node protocol
├── uhorse-channel/       # channel implementations
├── uhorse-config/        # unified config model
├── uhorse-llm/           # LLM client
└── ...
```

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
