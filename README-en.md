<p align="center">
  <strong>English</strong> | <a href="README.md">简体中文</a>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>uHorse v4.6.0 current formal release line</strong>
</p>

<p align="center">
  <em>Hub handles scheduling and channel intake, while Node executes locally and returns results; the current repository HEAD is now closed as the formal `v4.6.0` release, with `uhorse-hub` and `uhorse-node-desktop` as the primary deliverables.</em>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#current-status">Current Status</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#documentation-index">Docs</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.6.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-released-green" alt="Status">
</p>

---

## Overview

The current public release line is **v4.6.0 Hub-Node mainline**.

Core components and primary deliverables:

- `uhorse-hub`: cloud-side control plane for Node access, task scheduling, Web API, approval endpoints, and DingTalk Stream intake.
- `uhorse-node-runtime`: the real Node runtime implementation, including reconnect, workspace protection, permissions, browser execution, approval requests, and task execution.
- `uhorse-node-desktop`: the recommended local desktop form factor for Node, delivered as a `bin/ + web/` archive plus a macOS `.pkg` and a Windows installer.
- `uhorse-protocol`: protocol types shared by Hub and Node, including `TaskAssignment`, `TaskResult`, `ApprovalRequest`, and `ApprovalResponse`.
- `uhorse-config`: unified Hub config model covering `server`, `channels`, `security`, `llm`, and related sections.

The `v4.6.0` capabilities already visible and validated in the repository include:

- DingTalk natural-language requests can enter the Hub → Node pipeline and, in controlled cases, be planned into a `BrowserCommand`.
- Hub locally validates browser targets and rejects `file://`, localhost, private-network, and other out-of-bound targets.
- Node Desktop and the runtime support browser-capability routing, so browser tasks are dispatched to nodes that declare `CommandType::Browser`; for DingTalk requests such as “open a webpage”, the mainline contract now plans them as `BrowserCommand::OpenSystem` so they execute with host system browser semantics.
- DingTalk now prefers attaching a `🤔思考中` reaction to the user's original message and automatically recalling it after task completion, failure, or cancellation; if reaction attach fails, the runtime falls back to the existing processing path on a best-effort basis.
- `memory / agent / skill` now support the layered chain `global / tenant / enterprise / department / role / user / session`; `memory_context_chain` reads from shared to private, while `visibility_chain` resolves from private back to shared.
- task context and runtime sessions now explicitly distinguish the stable `execution_workspace_id` from the Hub-side logical `collaboration_workspace_id` / `CollaborationWorkspace`; the former defines the real execution boundary, while the latter only carries collaboration context and default binding.
- the runtime API and Web UI expose source-aware metadata through `source_layer` and `source_scope`, so same-name resources from different sources can be distinguished.
- Node Desktop is delivered as a `bin/ + web/` archive, a macOS `.pkg`, a Windows installer, matching smoke coverage, and GitHub release / nightly artifacts, not as a native `.app/.dmg`, code signing, notarization, `.msi`, or Linux native installer.

These docs are aligned to what is **actually implemented and exercised in the repository today**. They no longer treat `/health/live`, `/health/ready`, `/api/v1/auth/*`, or `/api/v1/messages` as the current mainline, and they do not describe `v4.6.0` as a return to the old monolithic Agent platform.

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
| Layered `memory / agent / skill` scopes | ✅ | the runtime now organizes sharing and isolation across `global / tenant / enterprise / department / role / user / session` scopes |
| Runtime session / collaboration workspace APIs | ✅ | `/api/v1/sessions*` now return `namespace`, `memory_context_chain`, `visibility_chain`, and `collaboration_workspace` |
| Source-aware runtime / UI | ✅ | runtime pages such as Skills and Settings expose `source_layer` and `source_scope` so same-name multi-source resources can be distinguished |
| Node Desktop packaging and smoke | ✅ | the current delivery path is a `bin + web` archive plus a macOS `.pkg` and a Windows installer, and CI / release / nightly all publish matching artifacts; `.app/.dmg`, `.msi`, and Linux native installers are outside the current boundary |
| Real local integration test | ✅ | `test_local_hub_node_roundtrip_file_exists` and `test_local_hub_node_roundtrip_file_write` cover real Hub + Node + WebSocket roundtrips |
| Auth rejection path | ✅ | `test_local_hub_rejects_node_with_mismatched_auth_token` covers token / registration `node_id` mismatch |
| DingTalk Stream integration | ✅ | Stream mode is the recommended path; Node Desktop can establish runtime bindings through pairing, and `channels.dingtalk.notification_bindings` is now only a compatibility seed/fallback |
| DingTalk browser planning path | ✅ | Hub now allows controlled `BrowserCommand` planning and dispatches browser work to nodes that declare `CommandType::Browser` |
| Agent Browser Skill install regression | ✅ | `test_agent_browser_natural_language_install_flow_returns_chinese_hint` now covers natural-language install, SkillHub install, and Chinese reply hints |
| Online Skill install compatibility | ✅ | `POST /api/v1/skills/install` and the DingTalk install thin entrypoint now accept `.zip` / `.tar.gz`, and support `skill.yaml` Python Skills via generated `skill.toml` plus `.venv` dependency installation |

## Quick Start

### 1. Build the mainline binaries

```bash
git clone https://github.com/kts-kris/uHorse
cd uHorse
cargo build --release -p uhorse-hub -p uhorse-node -p uhorse-node-desktop
```

Primary outputs:

- `target/release/uhorse-hub`
- `target/release/uhorse-node`
- `target/release/uhorse-node-desktop`

If you want prebuilt mainstream-platform packages, use the GitHub Release / nightly `uhorse-hub` archives and the `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer artifacts.

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

### 4. Default quick regression entrypoints

The recommended default quick regression entrypoints are:

```bash
make test-quick
make skill-install-smoke
```

Where:

- `make test-quick` now runs release build, real Hub-Node roundtrip, the Agent Browser Skill install regression, Node workspace check, and Hub Docker smoke by default.
- `make skill-install-smoke` runs `test_agent_browser_natural_language_install_flow_returns_chinese_hint` directly to validate the “帮我安装 Agent Browser 技能” natural-language install path, SkillHub installation, and Chinese reply hint generation.

### 5. Enable auth and approvals

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

- enable `notifications_enabled` locally
- enable `show_notification_details` if you also want richer notification content
- enable `mirror_notifications_to_dingtalk` if the local notification should also be forwarded to DingTalk
- start pairing from Node Desktop and confirm the runtime binding in DingTalk with the pairing code
- only configure `channels.dingtalk.notification_bindings` when you need compatibility seed/fallback behavior
- if the running Node and the newly saved config differ, Settings / Dashboard will show that a restart is required before the new workspace and runtime config take effect

### 5. Install a Skill online (HTTP / DingTalk)

The current online install path now supports these compatibility cases:

- `POST /api/v1/skills/install` accepts both `.tar.gz` and `.zip` archives
- DingTalk supports both `安装技能 <package> <download_url> [version]` and the flow where a user uploads a `.zip` package first, then follows up with “帮我安装这个技能”
- for Python Skills that only ship `skill.yaml` plus `src/main.py` / `main.py`, Hub generates `skill.toml` automatically
- if the package includes `requirements.txt`, Hub creates `.venv`, installs dependencies, and then refreshes the runtime Skill registry

See [SKILLS-en.md](SKILLS-en.md), [API-en.md](API-en.md), and [CHANNELS-en.md](CHANNELS-en.md) for the exact API and boundary details.

### 6. Submit a minimal task

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
- Hub Web API plus DingTalk / browser planning validation: `crates/uhorse-hub/src/web/mod.rs`
- Hub WebSocket auth and registration: `crates/uhorse-hub/src/web/ws.rs`
- Hub scheduler: `crates/uhorse-hub/src/task_scheduler.rs`
- Node CLI entrypoint: `crates/uhorse-node/src/main.rs`
- Node runtime: `crates/uhorse-node-runtime/src/node.rs`
- Node browser execution and command dispatch: `crates/uhorse-node-runtime/src/executor.rs`
- Node Desktop desktop host: `crates/uhorse-node-desktop/src/main.rs`
- Local integration tests: `crates/uhorse-hub/tests/integration_test.rs`

## Documentation Index

| Document | Description |
|----------|-------------|
| [CHANGELOG-en.md](CHANGELOG-en.md) | `v4.6.0` release facts, documentation sync notes, and explicit non-goals |
| [INSTALL-en.md](INSTALL-en.md) | current Hub-Node install path plus the Node Desktop archive / smoke boundary |
| [API-en.md](API-en.md) | current implemented Hub-Node API surface |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | local dual-process setup, JWT bootstrap, approval, and reconnect regression |
| [CONFIG-en.md](CONFIG-en.md) | unified config, legacy HubConfig, NodeConfig, and permission rules |
| [CHANNELS-en.md](CHANNELS-en.md) | current channel status, DingTalk Stream, browser planning path, and notification mirroring |
| [SKILLS-en.md](SKILLS-en.md) | online Skill install, package structure, and Python Skill compatibility details |
| [scripts/README.md](scripts/README.md) | mainline scripts, including Node Desktop package / smoke and CI / release aligned usage |
| [TESTING.md](TESTING.md) | package tests, workspace tests, `make test-quick` / `make skill-install-smoke`, and manual regression order |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | `v4.6.0` release notes plus current online Skill install compatibility notes |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4 Hub-Node deployment guide |
| [docs/architecture/v4.0-architecture-en.md](docs/architecture/v4.0-architecture-en.md) | v4 architecture details |

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
