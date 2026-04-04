## uHorse 4.5.1 发布

**发布日期**：2026-04-04

### 本次发布重点

`v4.5.1` 聚焦修复钉钉真实联调里暴露的两类消息回归：**session webhook 泄漏 `[Wait]`**，以及 **continuation planner 返回命令 JSON 时被误当成最终文本直接回给用户**。

本次版本重点不是扩展新能力，而是把 **钉钉回复体验**、**continuation 命令兼容解析** 与 **中英文发布文档口径** 一次性收口到当前 Hub-Node 主线中。

### 主要变更

- 修复 DingTalk session webhook 路径，最终回包不再向用户泄漏 `[Wait]`
- 兼容 continuation planner 返回 `{"action":"execute_command", ...}` 顶层写法，避免命令 JSON 原样回显给钉钉用户
- 首轮规划与 continuation 继续共用同一套命令 JSON 归一化逻辑，`execute_command` 会继续进入任务派发
- README / INSTALL / CHANNELS / CHANGELOG / RELEASE_NOTES 已同步更新到 `v4.5.1` 口径

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 当前正式交付边界仍是 `bin + web` archive、macOS `.pkg` 与 Windows installer；GitHub Release / nightly 继续提供这些多平台产物。

### 不包含内容

`v4.5.1` 明确 **不包含**：

- DingTalk 文本入口的 Skill refresh 命令；当前 refresh 只开放 HTTP API
- 对非 `skillhub` 来源的在线安装支持
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
