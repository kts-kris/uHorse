## uHorse 4.1.2 发布

**发布日期**：2026-03-29

### 本次发布重点

`v4.1.2` 的目标不是扩展新的产品线，而是把当前已经完成的 Hub-Node 主线、DingTalk 浏览器链路与 Node Desktop 交付边界正式收口为一版可发布的主线版本。

### 主要新增

- DingTalk 自然语言请求现在可以在受控场景下规划为 `BrowserCommand`
- Hub 会对浏览器目标执行本地安全校验，拒绝 `file://`、localhost、私网地址和其他越界目标
- `uhorse-node-runtime` 已接入正式浏览器执行路径
- `uhorse-node-desktop` 默认启用 `browser` feature，并通过 `CommandType::Browser` 参与能力路由
- GitHub release / nightly workflow 现在为 `uhorse-hub` 与 `uhorse-node-desktop` 生成主流平台 archive 产物
- `channels.dingtalk.notification_bindings` 已纳入当前主线说明，用于将稳定 `node_id` 绑定到 DingTalk `user_id`

### 主要变更

- Node Desktop 当前正式交付边界已固定为 `bin + web` archive
- README / INSTALL / CHANNELS / scripts / release 文档已统一到 `v4.1.2` 口径
- 每日构建与正式发布链路已统一使用 `Cargo.toml` 版本与 `CHANGELOG.md` 版本段作为发布事实源
- `memory / agent / skill` 的 4.1 叙事已升级为 `global / tenant / enterprise / department / role / user / session` 分层共享链
- 任务上下文与 runtime session 已显式区分稳定 `execution_workspace_id` 和 Hub 侧逻辑 `collaboration_workspace_id` / `CollaborationWorkspace`
- runtime API 与 Web UI 已通过 `source_layer`、`source_scope` 暴露来源感知信息，`/api/v1/sessions*` 也可返回 `namespace`、`memory_context_chain`、`visibility_chain` 与 `collaboration_workspace`

### 主交付物

本次发布的主交付物是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 继续以 `bin + web` archive 形式发布，面向主流平台提供 `.tar.gz` 或 `.zip` 包。

### 不包含内容

`v4.1.2` 明确 **不包含**：

- 原生 `.app/.dmg`、签名、公证、安装器或拖拽安装体验
- 旧时代 `agent / skill / memory` 独立平台的全面回归
- 将 legacy `uhorse` 单体路径恢复为主交付物

### 验证基线

发布前已完成并建议持续复核的验证基线包括：

```bash
cargo test --workspace
./scripts/package-node-desktop.sh
./scripts/desktop-smoke.sh
cargo build --release -p uhorse-hub -p uhorse-node-desktop
```

### 升级与获取方式

- 从源码构建：见 `README.md` 与 `INSTALL.md`
- 获取预编译产物：使用 GitHub Release / nightly 中的 `uhorse-hub` 与 `uhorse-node-desktop` archive
- 查看完整变更：见 `CHANGELOG.md` 与 `CHANGELOG-en.md`
