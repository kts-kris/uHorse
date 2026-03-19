# uHorse Channel Guide

This document only describes the **channel path that is actually wired into the current mainline runtime**.

The most important and recommended path today is:

- **DingTalk Stream mode**

The repository still contains Telegram, Slack, Discord, WhatsApp, Feishu, and WeCom channel modules, but the current `uhorse-hub` runtime and documentation focus on DingTalk.

## Table of Contents

- [Current channel status](#current-channel-status)
- [DingTalk Stream mode](#dingtalk-stream-mode)
- [Minimal config](#minimal-config)
- [What happens when Hub starts](#what-happens-when-hub-starts)
- [How messages enter the task pipeline](#how-messages-enter-the-task-pipeline)
- [Current DingTalk command allowlist](#current-dingtalk-command-allowlist)
- [Reply path](#reply-path)
- [Webhook route note](#webhook-route-note)
- [Relationship with LLMs and custom providers](#relationship-with-llms-and-custom-providers)
- [Testing and verification](#testing-and-verification)
- [Next steps](#next-steps)

---

## Current channel status

| Channel | Doc status | Runtime status |
|---------|------------|----------------|
| DingTalk | primary documented path | wired into main flow |
| Telegram | module exists | not the current mainline focus |
| Slack | module exists | not the current mainline focus |
| Discord | module exists | not the current mainline focus |
| WhatsApp | module exists | not the current mainline focus |
| Feishu / WeCom | module exists | not the current mainline focus |

If you are validating the current mainline, focus on DingTalk first.

---

## DingTalk Stream mode

For the current `uhorse-hub` runtime, the recommended DingTalk mode is:

- **Stream mode**
- Hub establishes a long-lived connection
- inbound messages do not require a public IP as the primary documented path
- webhook is no longer the main conceptual model

This is also the path aligned with the current README and config docs.

---

## Minimal config

Enable DingTalk in a unified config file:

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789
```

> DingTalk can only be initialized from the **unified config** path. The legacy `HubConfig` mode cannot initialize DingTalk.

---

## What happens when Hub starts

When `channels.enabled` contains `dingtalk`, Hub startup will:

1. load `[channels.dingtalk]`
2. initialize `DingTalkChannel`
3. start DingTalk in Stream mode
4. subscribe to inbound messages
5. convert inbound text into Hub tasks
6. reply results back to the original DingTalk conversation

---

## How messages enter the task pipeline

The current main flow is:

```text
DingTalk inbound message
    → DingTalkChannel
    → submit_dingtalk_task(...)
    → Hub::submit_task(...)
    → scheduled to an online Node
    → Node executes
    → Node sends TaskResult back
    → Hub reply_task_result(...)
    → reply to the original DingTalk conversation
```

So DingTalk messages do not stop at the channel layer. They enter the Hub-Node execution pipeline.

---

## Current DingTalk command allowlist

The current Hub runtime exposes a minimal DingTalk text-command allowlist for controlled execution:

- `list` / `ls`
- `search`
- `read` / `cat`
- `info`
- `exists`

These are converted into file-oriented tasks and executed by the Node inside its controlled workspace.

---

## Reply path

The current result handling keeps the full execution result and attempts to route it back to the original DingTalk session.

The current reply strategy is roughly:

- text output → reply directly
- JSON output → pretty-print then reply
- failure → reply with error text

So DingTalk is both the inbound entrypoint and the result return channel.

---

## Webhook route note

Even though Stream mode is the primary path, Hub still exposes compatibility / auxiliary webhook routes:

```text
GET  /api/v1/channels/dingtalk/webhook
POST /api/v1/channels/dingtalk/webhook
```

Important:

- this does not change the fact that Stream mode is the recommended mode
- deployment and runtime docs should no longer treat webhook as the primary path

---

## Relationship with LLMs and custom providers

DingTalk and LLM initialization are both driven by the unified Hub config, so they often appear together.

Example:

```toml
[llm]
enabled = true
provider = "custom-provider"
api_key = "your_api_key"
base_url = "https://api.example.com/v1"
model = "your-model"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

The current code supports custom model providers:

- `provider` may be any custom string
- unknown provider strings are treated as a **Custom provider**
- the client currently assumes an **OpenAI-compatible API**:
  - `POST {base_url}/chat/completions`
  - `Authorization: Bearer <api_key>`

So if you want to plug DingTalk into an internal enterprise model platform, that platform needs to expose an OpenAI-compatible endpoint.

---

## Testing and verification

### Start Hub and inspect logs

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

Watch for:

- DingTalk channel initialization
- Stream mode startup
- inbound subscription activity

### Verify together with Node

```bash
./target/release/uhorse-node --config node.toml --log-level info
curl http://127.0.0.1:8765/api/nodes
```

### Verified local baseline test

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

This does not require real DingTalk credentials, but it proves that the Hub-Node execution pipeline is closed locally.

---

## Next steps

- [CONFIG-en.md](CONFIG-en.md): unified vs legacy config boundary
- [README-en.md](README-en.md): project overview
- [TESTING.md](TESTING.md): test and validation commands
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md): v4.0 deployment guide
