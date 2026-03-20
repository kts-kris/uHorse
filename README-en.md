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
  <em>Hub schedules work, Node executes locally, and DingTalk Stream provides the enterprise message entrypoint.</em>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#current-status">Current Status</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#documentation-index">Docs</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.0.0--alpha.1-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-yellow" alt="Status">
</p>

---

## Overview

The current mainline of this repository is the **v4.0 Hub-Node architecture**:

- `uhorse-hub`: cloud-side control plane for node access, task scheduling, Web API, DingTalk intake, and result replies.
- `uhorse-node`: local execution node that runs commands inside a controlled workspace and reports results back.
- `uhorse-protocol`: message protocol between Hub and Node.
- `uhorse-channel`: the current Hub runtime is wired for **DingTalk Stream mode**.
- `uhorse-config`: shared configuration model used by Hub for DingTalk / LLM / base service settings.

These docs are now aligned to what is actually implemented in the repository. They no longer describe the old monolithic `uhorse` runtime, old health endpoints, or old `OPENCLAW_*` variables as the primary path.

## Current Status

| Capability | Status | Notes |
|------------|--------|-------|
| Local Hub startup | ✅ | `uhorse-hub` serves `/api/health`, `/api/nodes`, and `/ws` |
| Local Node startup | ✅ | `uhorse-node` loads `node.toml` and connects to Hub |
| Hub → Node dispatch | ✅ | task submission triggers scheduling |
| Node → Hub result return | ✅ | Node sends full `NodeToHub::TaskResult` |
| Local roundtrip verification | ✅ | covered by `test_local_hub_node_roundtrip_file_exists` |
| DingTalk Stream integration | ✅ | Stream mode is the intended path; no public webhook is required for message intake |
| DingTalk natural-language planning | ✅ | Hub uses an LLM to plan natural-language requests into locally validated `FileCommand` / `ShellCommand` values |
| DingTalk natural-language result summary | ✅ | Hub summarizes `CompletedTask` results with an LLM and falls back to structured text when summarization fails |
| Node workspace protection and git automation | ✅ | Node stays inside the workspace by default, blocks dangerous git commands, and auto-stages newly created files |
| Real DingTalk tenant verification | ✅ | the message intake and original-conversation reply path have been validated with a real enterprise tenant |

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

### 2. Generate default config files

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

If you only want the smallest local roundtrip setup, use the minimal configs below.

### 3. Minimal local roundtrip config

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

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = ""
```

### 4. Start Hub and Node

Terminal 1:

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

Terminal 2:

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

### 5. Verify connectivity

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/api/nodes
```

If `/api/nodes` returns an online node list, Hub and Node are connected.

### 6. Run the real local roundtrip integration test

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

This test starts a real Hub, a real WebSocket server, and a real Node, then verifies a file command is dispatched to the Node and returned back to the Hub.

## DingTalk Stream Mode

The current `uhorse-hub` runtime already wires DingTalk into the main execution flow:

- Recommended mode: **Stream mode**
- Benefit: no public IP or public webhook is required for inbound message delivery
- Hub first uses an LLM to plan natural-language requests into locally validated `FileCommand` / `ShellCommand` values
- Local validation keeps paths inside the workspace and rejects dangerous git commands
- Hub prefers LLM-generated result summaries before replying to the original DingTalk conversation, and falls back to structured text if summarization fails

To enable DingTalk, use a unified config file. See [CONFIG-en.md](CONFIG-en.md) and [CHANNELS-en.md](CHANNELS-en.md).

## Architecture

```text
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  • Web API: /api/health /api/nodes /api/*   │
│  • WebSocket: /ws                            │
│  • Task Scheduler                            │
│  • DingTalk Stream / result reply            │
└──────────────────────────────────────────────┘
                      │
                      │ WebSocket
                      ▼
┌──────────────────────────────────────────────┐
│                 uhorse-node                  │
│  • Workspace                                 │
│  • Permission Manager                        │
│  • Command Executor                          │
│  • TaskResult return                         │
└──────────────────────────────────────────────┘
```

### Primary source entrypoints

- Hub startup and unified config loading: `crates/uhorse-hub/src/main.rs`
- Hub Web API and DingTalk routing: `crates/uhorse-hub/src/web/mod.rs`
- Hub scheduling core: `crates/uhorse-hub/src/hub.rs`
- Node startup entrypoint: `crates/uhorse-node/src/main.rs`
- Node execution and result return: `crates/uhorse-node/src/node.rs`
- Local roundtrip test: `crates/uhorse-hub/tests/integration_test.rs`

## Documentation Index

| Document | Description |
|----------|-------------|
| [INSTALL-en.md](INSTALL-en.md) | installation and binary build |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | local Hub-Node development and startup |
| [CONFIG-en.md](CONFIG-en.md) | actual config structure and examples |
| [CHANNELS-en.md](CHANNELS-en.md) | current channel status, focused on DingTalk Stream |
| [TESTING.md](TESTING.md) | build, test, and local roundtrip verification |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4.0 Hub-Node deployment guide |
| [deployments/DEPLOYMENT.md](deployments/DEPLOYMENT.md) | deployment overview and migration notes |

## Workspace Layout

```text
crates/
├── uhorse-hub/        # cloud hub
├── uhorse-node/       # local node
├── uhorse-protocol/   # Hub-Node protocol
├── uhorse-channel/    # channel implementations (current Hub runtime focuses on DingTalk)
├── uhorse-config/     # unified configuration
├── uhorse-llm/        # LLM client
└── ...                # other 3.x / 4.0 related modules
```

## License

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
