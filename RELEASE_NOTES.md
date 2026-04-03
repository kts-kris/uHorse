## uHorse 4.5.0 发布

**发布日期**：2026-04-02

### 本次发布重点

`v4.5.0` 在上一轮已经完成实现并通过测试的在线安装 Skill 能力基础上，进一步把 **Agent Browser Skill 安装自动化回归**、**默认快速回归入口** 与 **中英文文档 / release 口径统一** 收口为正式发布事实。

本次版本重点不是扩大 Skill 平台边界，而是把 **默认回归入口**、**Agent Browser Skill 自然语言安装 smoke** 与 **README / INSTALL / TESTING / scripts / release 文档同步** 一次性收口到当前 Hub-Node 主线中。

### 主要变更

- 新增 `make skill-install-smoke`，用于单独运行 Agent Browser Skill 安装 smoke 回归
- `make test-quick` 现在默认包含 Agent Browser Skill 安装自动化回归
- `uhorse-hub` 新增 `test_agent_browser_natural_language_install_flow_returns_chinese_hint`，覆盖“帮我安装 Agent Browser 技能”的自然语言安装、SkillHub 安装与中文提示
- README / INSTALL / TESTING / CHANNELS / CONFIG / scripts / CHANGELOG 已同步更新到 `v4.5.0` 口径，并补齐默认回归入口说明
- 在线安装、运行时 refresh、DingTalk 文本安装入口与 `[[channels.dingtalk.skill_installers]]` 白名单控制继续保留为当前主线事实

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 当前正式交付边界仍是 `bin + web` archive、macOS `.pkg` 与 Windows installer；GitHub Release / nightly 继续提供这些多平台产物。

### 不包含内容

`v4.5.0` 明确 **不包含**：

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
