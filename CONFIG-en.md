# uHorse Configuration Guide

This document only describes the configuration structures that are **actually consumed by the current codebase**, with emphasis on:

- the two runtime config modes of `uhorse-hub`
- `node.toml` for `uhorse-node`
- real DingTalk Stream and LLM fields

## Table of Contents

- [Configuration Modes Overview](#configuration-modes-overview)
- [Hub Configuration](#hub-configuration)
- [Node Configuration](#node-configuration)
- [DingTalk Stream Configuration](#dingtalk-stream-configuration)
- [LLM Configuration](#llm-configuration)
- [Validation Commands](#validation-commands)

---

## Configuration Modes Overview

`uhorse-hub` currently supports **two config modes**.

### Mode 1: Unified config

Use this when you need:

- DingTalk
- LLM
- the shared `uhorse-config` structure

Entrypoint:

```bash
./target/release/uhorse-hub --config hub.toml
```

Detection rule:

If the file contains any of the following sections, Hub treats it as a unified config:

- `[server]`
- `[database]`
- `[channels]`
- `[security]`
- `[logging]`
- `[observability]`
- `[scheduler]`
- `[tools]`
- `[llm]`

In the current code, unified config directly controls:

- Hub bind host and port (from `[server]`)
- DingTalk initialization (from `[channels.dingtalk]`)
- LLM initialization (from `[llm]`)

But **Hub-specific scheduler fields such as `max_nodes`, `heartbeat_timeout_secs`, `task_timeout_secs`, and `max_retries` are not read from unified config yet**. They still fall back to `HubConfig::default()`.

### Mode 2: legacy HubConfig

Use this when you need:

- a minimal Hub startup
- explicit control over Hub scheduler/runtime fields
- no DingTalk / LLM initialization

Example:

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

Important: legacy mode does **not** contain `[channels]` or `[llm]`, so it cannot initialize DingTalk or LLM.

---

## Hub Configuration

### Option A: unified config example

This is the best option for a real Hub runtime, especially when DingTalk Stream or LLM is needed.

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

### Option B: legacy HubConfig example

This is the smallest possible Hub config:

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

### Hub CLI arguments

```bash
./target/release/uhorse-hub --help
```

Current important flags:

- `--config`: config file path, default `hub.toml`
- `--log-level`: log level, default `info`
- `--host`: command-line mode only, default `0.0.0.0`
- `--port`: command-line mode only, default `8765`
- `--hub-id`: Hub ID, default `default-hub`

### Generate a default Hub config

```bash
./target/release/uhorse-hub init --output hub.toml
```

`init` generates a **unified config file**, not the legacy `HubConfig` shape.

---

## Node Configuration

`uhorse-node` only reads `NodeConfig`; it does not have unified vs legacy modes.

### Minimal Node config

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

### More complete Node example

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

### Node workspace protection

The current `uhorse-node` runtime keeps execution inside `workspace_path` and additionally enables these defaults:

- `git_protection_enabled = true`: block dangerous git commands
- `watch_workspace = true`: watch for newly created files in the workspace
- `auto_git_add_new_files = true`: run local `git add` for newly created files
- `require_git_repo = true`: require the workspace itself to already be a git repository
- `internal_work_dir = ".uhorse"`: internal temp-code directory that the watcher skips by default
- file writes automatically create missing parent directories inside the workspace, but still reject any parent-path escape outside `workspace_path`
- browser commands keep `OpenSystem` and `Navigate` as separate semantics; the current DingTalk “open webpage” path lands on `OpenSystem`

### Node CLI arguments

```bash
./target/release/uhorse-node --help
```

Current important flags:

- `--config`: config path, default `node.toml`
- `--log-level`: log level, default `info`
- `--hub-url`: default `ws://localhost:8765/ws`
- `--workspace`: default `.`
- `--name`: default `uHorse-Node`

### Node subcommands

```bash
./target/release/uhorse-node init --output node.toml
./target/release/uhorse-node check --workspace /path/to/workspace
```

---

## DingTalk Stream Configuration

The recommended and documented path for the current `uhorse-hub` runtime is **Stream mode**.

### Minimal DingTalk config

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

### Notes

- The main runtime path is **Stream mode** and does not depend on a public webhook to receive inbound messages.
- Hub still exposes `GET/POST /api/v1/channels/dingtalk/webhook` for compatibility and auxiliary testing.
- To mirror Node Desktop local notifications to DingTalk, the main path is to enable pairing, start binding from Node Desktop, and confirm it in DingTalk; `channels.dingtalk.notification_bindings` is kept only as a compatibility seed/fallback.
- `[[channels.dingtalk.skill_installers]]` only restricts the DingTalk text install entrypoint; it does not restrict the HTTP `POST /api/v1/skills/install` API.
- The default quick regression path now includes the Agent Browser Skill install smoke, and you can run it directly with `make skill-install-smoke`.
- Allowlist matching supports `user_id` / `staff_id` and may optionally require `corp_id`.
- DingTalk text is first planned by the LLM into a single safe command; file operations, shell commands, and controlled `BrowserCommand` flows are all part of the current mainline.
- But `安装技能 <package> <download_url> [version]` / `install skill ...` goes through the dedicated Skill install thin entrypoint instead of general natural-language command planning.
- For requests such as “open a webpage”, the current mainline prefers `BrowserCommand::OpenSystem` instead of automated browser `Navigate`.
- Hub validates path scope locally before dispatch and rejects dangerous git commands.

### What happens when enabled

When `channels.enabled` contains `dingtalk`, Hub startup will:

1. initialize `DingTalkChannel`
2. subscribe to inbound DingTalk messages
3. send inbound natural language to the LLM for safe command planning
4. submit only locally validated commands as Hub tasks
5. prefer an LLM-generated result summary before replying through `session_webhook`; when the webhook is unavailable or expired, fall back to group or personal message sending

The current mainline has already been validated once with a real enterprise tenant:

- invalid or unsafe requests return immediate errors
- valid requests route results back to the original conversation

---

## LLM Configuration

The current Hub initializes `OpenAIClient` from the unified `[llm]` section.

### Example

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

### Field reference

| Field | Meaning |
|-------|---------|
| `enabled` | enable or disable LLM initialization |
| `provider` | provider identifier |
| `api_key` | API key |
| `base_url` | API base URL |
| `model` | model name |
| `temperature` | sampling temperature |
| `max_tokens` | max output tokens |
| `system_prompt` | system prompt |

If `enabled = false`, Hub skips LLM initialization during startup.

---

## Validation Commands

### Generate default configs

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### Check Node workspace access

```bash
./target/release/uhorse-node check --workspace .
```

### Start Hub and Node

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
./target/release/uhorse-node --config node.toml --log-level info
```

### Health and connectivity checks

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/api/nodes
```

Note: `uhorse-hub` only exposes the unified-config health route when `server.health.enabled = true`; if you do not override it, the default remains:

```text
/api/health
```

---

## Recommendations

- Use **unified config** when you need DingTalk or LLM.
- Use **legacy HubConfig + NodeConfig** when you want the smallest local Hub-Node roundtrip.
- Do not assume unified config already covers every Hub-specific scheduler knob; the current code still has a boundary between unified runtime config and legacy Hub tuning fields.

See also:

- [README-en.md](README-en.md)
- [LOCAL_SETUP.md](LOCAL_SETUP.md)
- [CHANNELS-en.md](CHANNELS-en.md)
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)
