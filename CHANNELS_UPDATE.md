# 企业通道支持更新

本文档记录企业通道接入状态，当前事实以 `README.md`、`CHANNELS.md`、`CONFIG.md` 和 `API.md` 为准。

## 当前状态

| 通道 | 当前状态 | 说明 |
|------|----------|------|
| DingTalk | 主生产路径 | Stream 入站、Hub 任务规划、原消息 reaction、AI Card / transient handle 与结果回传均已接入主链路 |
| Feishu | 最小第二样本 | 支持 webhook challenge、message event prepared inbound，以及基于 `ReplyContext` 的原消息回包 |
| WeWork | 配置 / 初始化样本 | 支持统一配置初始化，但尚未进入 Hub prepared inbound 主线 |
| Telegram / Slack / Discord / WhatsApp | 模块保留 | 非当前 `v4.6.0` Hub 主线验证重点 |

## 已落地能力

### DingTalk

- 通过统一配置 `[channels.dingtalk]` 初始化。
- 当前推荐使用 Stream 模式接收入站消息。
- 自然语言消息可进入 Hub → Node 任务链路。
- 处理中状态优先使用 AI Card；未命中时优先在原消息上贴 `🤔思考中` reaction，并在任务完成、失败或取消后 best-effort recall / clear。
- Skill 安装薄入口支持 `安装技能 <package> <download_url> [version]`，并受 `[[channels.dingtalk.skill_installers]]` 白名单约束。

### Feishu

- 通过统一配置 `[channels.feishu]` 初始化。
- `GET /api/v1/channels/feishu/webhook` 返回 readiness 文本。
- `POST /api/v1/channels/feishu/webhook` 支持 challenge 响应。
- message event 可被预处理为 `PreparedInboundTurn` 并进入 Hub 调度主线。
- 普通回包走 `ReplyContext` + `Channel::reply_via_context(...)`，优先使用原始 `message_id` 调用 Feishu reply API。

### WeWork

- 通过统一配置 `[channels.wework]` 初始化。
- 当前不作为 Hub prepared inbound 主线验证对象。

## 配置示例

### DingTalk

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

### Feishu

```toml
[channels]
enabled = ["feishu"]

[channels.feishu]
app_id = "your-feishu-app-id"
app_secret = "your-feishu-app-secret"
# encrypt_key = "your-feishu-encrypt-key"
# verify_token = "your-feishu-verify-token"
```

### WeWork

```toml
[channels]
enabled = ["wework"]

[channels.wework]
corp_id = "your-wework-corp-id"
agent_id = 123456789
secret = "your-wework-secret"
# token = "your-wework-token"
# encoding_aes_key = "your-wework-encoding-aes-key"
```

## 验证入口

```bash
cargo test -p uhorse-channel
cargo test -p uhorse-hub test_dispatch_reply_via_context_uses_generic_channel_reply_path -- --nocapture
cargo test -p uhorse-hub test_prepare_feishu_inbound_and_submit_turn_dispatches_assignment -- --nocapture
cargo test -p uhorse-hub session_key_from_reply_context -- --nocapture
```

更多主线说明见：

- [CHANNELS.md](CHANNELS.md)
- [CONFIG.md](CONFIG.md)
- [API.md](API.md)
- [TESTING.md](TESTING.md)
