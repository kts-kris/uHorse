## uHorse 4.4.0 发布

**发布日期**：2026-04-02

### 本次发布重点

`v4.4.0` 把上一轮已经完成实现并通过测试的在线安装 Skill 能力正式收口为 release 事实：Hub 现在可以在运行时目录在线安装 / refresh Skill，DingTalk 也提供了受控的文本安装入口，并且安装权限可以被限制到指定账号。

本次版本重点不是扩大 Skill 平台边界，而是把 **在线安装**、**运行时 refresh**、**DingTalk 白名单权限控制** 和 **文档 / release 口径统一** 一次性收口到当前 Hub-Node 主线中。

### 主要变更

- Hub 新增 `POST /api/v1/skills/install`，支持把 Skill 包安装到运行时目录并在安装后立即刷新 registry
- Hub 新增 `POST /api/v1/skills/refresh`，支持不重启进程重载运行时 Skill
- DingTalk 新增文本安装命令：`安装技能 <package> <download_url> [version]` / `install skill <package> <download_url> [version]`
- 统一配置新增 `[[channels.dingtalk.skill_installers]]`，可按 `user_id` / `staff_id` 并可选叠加 `corp_id` 控制 DingTalk 安装入口
- DingTalk 安装入口会在下载前先校验授权，未授权账号会被直接拒绝，不再误入后续下载 / 安装路径
- 在线安装当前只接受 `source = "skillhub"` 的 Skill 包，并拒绝覆盖已存在的 Skill 目录
- README / INSTALL / CHANNELS / CONFIG / SKILLS / API / CHANGELOG 已同步更新到 `v4.4.0` 口径

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 当前正式交付边界仍是 `bin + web` archive、macOS `.pkg` 与 Windows installer；GitHub Release / nightly 继续提供这些多平台产物。

### 不包含内容

`v4.4.0` 明确 **不包含**：

- DingTalk 文本入口的 Skill refresh 命令；当前 refresh 只开放 HTTP API
- 对非 `skillhub` 来源的在线安装支持
- 覆盖式安装已存在 Skill
- 原生 `.app/.dmg`、签名、公证、`.msi`、Linux 原生安装器或拖拽安装体验
- 旧时代 `agent / skill / memory` 独立平台的全面回归

### 验证基线

本次发布前已完成并确认通过的基线包括：

```bash
cargo test -p uhorse-config
cargo test -p uhorse-hub
```

### 升级与获取方式

- 从源码构建：见 `README.md` 与 `INSTALL.md`
- 获取预编译产物：使用 GitHub Release / nightly 中的 `uhorse-hub` archive，以及 `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer
- 查看完整变更：见 `CHANGELOG.md` 与 `CHANGELOG-en.md`
