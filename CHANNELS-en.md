# uHorse Channel Guide

This document only describes the **channel path that is actually wired into the current `v4.1.2` mainline runtime**.

The most important and recommended path today is:

- **DingTalk Stream mode**

The repository still contains Telegram, Slack, Discord, WhatsApp, Feishu, and WeCom channel modules, but the current `uhorse-hub` runtime and documentation focus on DingTalk.

## Table of Contents

- [Current channel status](#current-channel-status)
- [DingTalk Stream mode](#dingtalk-stream-mode)
- [Minimal config](#minimal-config)
- [What happens when Hub starts](#what-happens-when-hub-starts)
- [How messages enter the task pipeline](#how-messages-enter-the-task-pipeline)
- [DingTalk natural-language planning and local validation](#dingtalk-natural-language-planning-and-local-validation)
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

[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"
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
    → plan_dingtalk_command(...)
    → Hub::submit_task(...)
    → scheduled to an online Node
    → Node executes
    → Node sends TaskResult back
    → Hub reply_task_result(...)
    → summarize_task_result(...)
    → reply to the original DingTalk conversation
```

So DingTalk messages do not stop at the channel layer. They first go through LLM planning, then enter the Hub-Node execution pipeline.

## Source-aware runtime

Under the current `v4.1.2` mainline wording, channel input that enters the Hub task pipeline also enters a runtime view that carries source metadata.

The goal is not to turn DingTalk into a management surface for `memory / agent / skill`, but to let the runtime distinguish where a resource came from and what sharing or isolation boundary it should follow.

The minimum public-facing semantics are:

- `source_layer`: identifies the source layer.
- `source_scope`: identifies the source scope and its sharing / isolation boundary.

This supports the current source-aware runtime / UI behavior without changing DingTalk's role as the channel entrypoint and result return path.

---

## DingTalk natural-language planning and local validation

The current `uhorse-hub` runtime no longer limits DingTalk text to a fixed command allowlist. Instead it:

1. reads the user's original natural-language request
2. asks the LLM to plan a single `Command`
3. only accepts `FileCommand`, `ShellCommand`, or a controlled `BrowserCommand`
4. validates file paths locally and rejects `..` or out-of-workspace absolute paths
5. validates browser targets locally and only allows public `http/https` URLs while rejecting `file://`, localhost, private-network, and other out-of-bound targets
6. blocks dangerous git commands such as `git reset --hard`, `git clean -fd`, and `git push --force`

If the LLM returns invalid JSON, an out-of-workspace path, an invalid browser target, or a dangerous command, Hub rejects it before anything is dispatched to the Node.

---

## Reply path

If Node Desktop local notification mirroring is enabled, the current mainline also supports a `channels.dingtalk.notification_bindings` mapping from stable `node_id` values to DingTalk `user_id` values so the “node notification -> DingTalk user” path can be completed.

The current result handling keeps the full execution result and tries to get back to the original DingTalk session in this order:

- prefer `session_webhook` when it is still valid
- fall back to group-message sending via `conversation_id`
- fall back to direct personal sending via `sender_user_id`

The reply-content strategy is:

- prefer an LLM-generated natural-language summary based on `CompletedTask`
- fall back to structured text when summarization fails
- return immediate error text when planning or local validation fails

The current mainline has already been validated with a real enterprise tenant: unsafe requests return immediate errors, and valid file / shell requests plus controlled browser requests are routed back to the original conversation.

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
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md): v4 deployment guide
