## uHorse 4.1.3 发布

**发布日期**：2026-03-29

### 本次发布重点

`v4.1.3` 是基于 `v4.1.2` 的正式发布收口补丁版本，不扩展新的产品线，目标是把当前 HEAD 上已经完成的仓库入口修正、包元数据对齐、验证基线结果与正式 Release 事实重新统一起来。

功能面延续 `v4.1.2` 的当前 Hub-Node 主线、DingTalk 浏览器链路与 Node Desktop 交付边界；`v4.1.3` 新增的重点是**发布事实对齐**，而不是新的主线能力扩展。

### 主要变更

- README / INSTALL / CONTRIBUTING / 部署与附属文档中的官方仓库入口已统一指向当前真实仓库 `https://github.com/kts-kris/uHorse`
- `Cargo.toml` 的 `repository` 包元数据已与当前 GitHub 仓库一致
- 已补跑并确认以下正式发布验证基线通过：
  - `cargo test --workspace`
  - `./scripts/package-node-desktop.sh`
  - `./scripts/desktop-smoke.sh`
  - `cargo build --release -p uhorse-hub -p uhorse-node-desktop`
- 当前 HEAD 的正式发布事实已收口为 `v4.1.3`，避免已发布 `v4.1.2` tag 与后续仓库入口 / 元数据修正脱节

### 当前主交付物

本次发布的主交付物仍然是：

- `uhorse-hub`
- `uhorse-node-desktop`

其中 `uhorse-node-desktop` 继续以 `bin + web` archive 形式发布；GitHub Release / nightly 继续提供多平台 archive 产物。

### 不包含内容

`v4.1.3` 明确 **不包含**：

- 原生 `.app/.dmg`、签名、公证、安装器或拖拽安装体验
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
- 获取预编译产物：使用 GitHub Release / nightly 中的 `uhorse-hub` 与 `uhorse-node-desktop` archive
- 查看完整变更：见 `CHANGELOG.md` 与 `CHANGELOG-en.md`
