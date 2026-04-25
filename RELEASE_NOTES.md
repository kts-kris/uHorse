## uHorse 4.6.0 发布

**发布日期**：2026-04-10

### 本次发布重点

`v4.6.0` 聚焦补齐 DingTalk 的原消息处理中交互：用户发出消息后，Hub 会优先在**原消息**上贴一个 `🤔思考中` reaction；当任务完成、失败或取消后，再自动 recall。

本次版本重点不是改写现有 Hub → runtime → Node → 回传主链路，而是在既有 reply handle 生命周期中接入 reaction，并保持 AI Card、session webhook、群消息和单聊最终回复路径不变。

### 主要变更

- 新增 DingTalk 原消息 `🤔思考中` reaction attach / recall 能力
- `uhorse-channel` 现在会透传 DingTalk 原始 `message_id`，供 Hub 在处理中句柄阶段直接使用
- Hub reply handle 生命周期新增 `Reaction` 变体，AI Card 仍优先，reaction attach 失败时会自动回退到现有路径
- 任务取消时也会清理处理中句柄：reaction 会 recall，legacy transient ack 会 clear，AI Card 会以“任务已取消。”收尾
- README / INSTALL / CHANNELS / CHANGELOG / RELEASE_NOTES 已同步更新到 `v4.6.0` 口径
- 当前文档已补齐在线 Skill 安装兼容性：`.zip` / `.tar.gz`、DingTalk 附件追装、`skill.yaml` Python Skill 自动生成 `skill.toml` 与 `requirements.txt` 自动安装依赖
- 当前主线已补齐最小 multi-channel reply-context 抽象，并以 Feishu 作为第二样本验证 webhook challenge、message event prepared inbound 与原消息 reply 回包

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 当前正式交付边界仍是 `bin + web` archive、macOS `.pkg` 与 Windows installer；GitHub Release / nightly 继续提供这些多平台产物。

### 不包含内容

`v4.6.0` 明确 **不包含**：

- DingTalk 文本入口的 Skill refresh 命令；当前 refresh 只开放 HTTP API
- 对非 `skillhub` 来源的在线安装支持
- 将 DingTalk AI Card / reaction / transient handle 强行泛化为跨通道处理中句柄 trait
- 将 WeWork、Telegram、Slack 等通道一次性接入 Hub prepared inbound 主线
- 覆盖式安装已存在 Skill
- 原生 `.app/.dmg`、签名、公证、`.msi`、Linux 原生安装器或拖拽安装体验
- 旧时代 `agent / skill / memory` 独立平台的全面回归

### 验证基线

本次发布前已完成并确认通过的基线包括：

```bash
cargo test -p uhorse-hub test_parse_planned_command_accepts_action_execute_command_tag -- --nocapture
cargo test -p uhorse-hub test_parse_next_step_response_parses_action_execute_command_as_submit_task -- --nocapture
cargo test -p uhorse-hub test_decide_dingtalk_action_extracts_action_execute_command_json_from_wrapped_text -- --nocapture
cargo test -p uhorse-hub test_send_or_finalize_dingtalk_reply_does_not_leak_wait_for_session_webhook_noop -- --nocapture
cargo test -p uhorse-hub test_reply_task_result_ignores_unsupported_transient_ack_clear -- --nocapture
```

### 升级与获取方式

- 从源码构建：见 `README.md` 与 `INSTALL.md`
- 获取预编译产物：使用 GitHub Release / nightly 中的 `uhorse-hub` archive，以及 `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer
- 查看完整变更：见 `CHANGELOG.md` 与 `CHANGELOG-en.md`
