## uHorse 4.3.0 发布

**发布日期**：2026-04-01

### 本次发布重点

`v4.3.0` 是当前 Hub-Node 主线与 Node Desktop 产品化能力的一次正式收口发布，重点不再只是仓库入口与发布事实对齐，而是把已经落地的桌面诊断、恢复、安装包与 DingTalk 绑定闭环一起纳入正式 release 口径。

本次版本延续当前 Hub-Node、DingTalk 浏览器链路与 Node Desktop 交付边界，并新增 **Settings 连接诊断 / 恢复能力** 与 **DingTalk pairing 绑定闭环修复** 两条已完成真实 acceptance 验证的主线能力。 

### 主要变更

- Node Desktop 新增 Settings 内的连接诊断 / 恢复能力，可直接查看连接健康度、认证前提、工作区状态、最近错误与最近日志，并执行最小恢复闭环
- 本地宿主新增 `GET /api/connection/diagnostics` 与 `POST /api/connection/recover` API，供桌面 Settings 页面复用
- DingTalk Stream 入站现在与 Web 路径统一先走 pairing 处理，绑定码消息会优先命中运行时绑定确认，不再误入普通任务文本链路
- Node Desktop DingTalk 绑定链路已完成真实 acceptance 验证：JWT 引导、pairing 确认、运行时绑定、连接诊断和已绑定状态展示全部打通
- Node Desktop 继续提供 `bin + web` archive，并新增 macOS `.pkg` 与 Windows installer；GitHub Release / nightly 会同步提供这些多平台产物
- README / TESTING / release 说明已同步更新到 `v4.3.0` 口径，明确 pairing 是当前主路径，`channels.dingtalk.notification_bindings` 仅作为兼容 seed/fallback

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 当前正式交付边界为 `bin + web` archive、macOS `.pkg` 与 Windows installer；GitHub Release / nightly 会同步提供这些多平台产物。

### 不包含内容

`v4.3.0` 明确 **不包含**：

- 原生 `.app/.dmg`、签名、公证、`.msi`、Linux 原生安装器或拖拽安装体验
- 旧时代 `agent / skill / memory` 独立平台的全面回归
- 将 legacy `uhorse` 单体路径恢复为主交付物

### 验证基线

本次发布前已完成并确认通过的基线包括：

```bash
cargo test --workspace
./scripts/package-node-desktop.sh
./scripts/desktop-smoke.sh
cargo build --release -p uhorse-hub -p uhorse-node-desktop
```

### 升级与获取方式

- 从源码构建：见 `README.md` 与 `INSTALL.md`
- 获取预编译产物：使用 GitHub Release / nightly 中的 `uhorse-hub` archive，以及 `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer
- 查看完整变更：见 `CHANGELOG.md` 与 `CHANGELOG-en.md`
