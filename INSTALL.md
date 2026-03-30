# uHorse 安装指南

本文档只描述 **当前仓库主线 `v4.1.3` Hub-Node 架构** 的真实安装路径。

当前最推荐、也最贴近已验证代码路径的安装方式是：

- 编译 `uhorse-hub` 与 `uhorse-node`
- 按需编译或下载 `uhorse-node-desktop`
- 生成 `hub.toml` 和 `node.toml`
- 按本地或部署场景分别启动 Hub 与 Node

> 注意：仓库里仍保留 `uhorse` 单体二进制以及 `install.sh`、`quick-setup.sh` 等脚本，但它们主要围绕旧单体路径，不是本文档的主推荐安装方式。

## 目录

- [系统要求](#系统要求)
- [从源码安装](#从源码安装)
- [可选：打包 Node Desktop](#可选打包-node-desktop)
- [可选：编译 legacy 单体二进制](#可选编译-legacy-单体二进制)
- [安装验证](#安装验证)
- [脚本说明](#脚本说明)
- [常见问题](#常见问题)
- [下一步](#下一步)

---

## 系统要求

### 最低要求

- **操作系统**：Linux、macOS、Windows（建议使用 WSL2）
- **Rust**：`1.78+`
- **内存**：至少 `512 MB`
- **磁盘**：至少 `200 MB`

### 常用依赖

- `cargo`：用于编译 Rust workspace
- `openssl`：部分环境下用于 TLS / 依赖编译 / 生成随机密钥
- `pkg-config`：Linux 下常见编译依赖

### 推荐环境

- `Rust stable`
- `2 GB+` 内存
- 可访问 Hub 的网络环境

---

## 从源码安装

这是当前最推荐的安装方式。

### 1. 克隆仓库

```bash
git clone https://github.com/kts-kris/uHorse
cd uHorse
```

### 2. 编译主线二进制

```bash
cargo build --release -p uhorse-hub -p uhorse-node -p uhorse-node-desktop
```

编译完成后，主线产物是：

- `target/release/uhorse-hub`
- `target/release/uhorse-node`
- `target/release/uhorse-node-desktop`

如果你不想本地编译，也可以直接从 GitHub Release / nightly 获取主流平台的 `uhorse-hub` 与 `uhorse-node-desktop` archive；其中 Node Desktop 额外提供 macOS `.pkg` 与 Windows installer `.exe`。

### 3. 生成默认配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 4. 按需调整配置

最小本地闭环通常只需要：

- `hub.toml`：Hub 地址、端口、调度参数
- `node.toml`：Node 名称、工作目录、Hub WebSocket 地址

完整字段见 [CONFIG.md](CONFIG.md)。如果你要验证 Node Desktop 真实通知镜像到钉钉，除了配置 DingTalk 凭据外，还需要在 Hub 侧补充 `channels.dingtalk.notification_bindings`。

### 5. 启动程序

终端 1：

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

终端 2：

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

---

## 可选：打包 Node Desktop

如果你要交付本地桌面客户端，而不是只运行宿主 API，可以直接使用仓库内置脚本。

`v4.1.3` 当前已经固定的交付边界是：**`bin + web` archive、安装器 smoke、CI / release / nightly artifacts**。这表示当前仓库主线已经覆盖可分发 archive、macOS `.pkg`、Windows installer 与对应 smoke 验证，但**仍不包含**原生 `.app/.dmg`、签名、公证、`.msi`、Linux 原生安装器。

可以直接使用仓库内置脚本：

```bash
./scripts/package-node-desktop.sh
```

默认 archive 产物：

- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>/bin/uhorse-node-desktop`
- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>/web/`
- 对应 `.tar.gz` 或 `.zip` 压缩包

如需原生安装包：

```bash
./scripts/package-node-desktop-macos-pkg.sh
./scripts/package-node-desktop-windows-installer.ps1
```

对应安装器产物：

- macOS：`target/node-desktop-package/uhorse-node-desktop-<version>-<target>.pkg`
- Windows：`target/node-desktop-package/uhorse-node-desktop-<version>-<target>-installer.exe`

若要验证 archive，可执行：

```bash
./scripts/desktop-smoke.sh
```

若要验证安装后目录，可执行：

```bash
./scripts/desktop-installer-smoke.sh <install-root>
```

Windows CI 中则使用：

```powershell
./scripts/desktop-installer-smoke.ps1 -InstallRoot <install-root>
```

当前 smoke 覆盖的是：

- Node Desktop 宿主 API
- 前端静态资源可访问性
- SPA 路由回退
- 安装后 `bin + web` 目录布局仍可运行

---

## 可选：编译 legacy 单体二进制

仓库里仍包含 `uhorse` 单体二进制目标。如果你只是要查看旧脚本、旧向导或做历史兼容验证，可以单独编译：

```bash
cargo build --release -p uhorse
```

产物路径：

- `target/release/uhorse`

> 但当前主线文档、README 和部署说明，默认都以 `uhorse-hub` + `uhorse-node` 为准。

---

## 安装验证

### 1. 检查二进制存在

```bash
./target/release/uhorse-hub --help
./target/release/uhorse-node --help
```

### 2. 检查 Node 工作空间可访问性

```bash
./target/release/uhorse-node check --workspace .
```

### 3. 启动后检查 Hub

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 4. 运行已验证的本地闭环测试

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_write -- --nocapture
```

这两条测试会真实启动：

- Hub
- WebSocket 服务
- Node
- 一个文件存在性命令 roundtrip
- 一个真实写文件 roundtrip（包含落盘与 structured file_operation 返回）

### 5. 验证 Node Desktop `v4.1.3` 交付边界

如果你正在验收 `v4.1.3` 的 Node Desktop 交付件，请额外执行：

```bash
./scripts/package-node-desktop.sh
./scripts/desktop-smoke.sh
```

如需继续验证原生安装包链路：

```bash
./scripts/package-node-desktop-macos-pkg.sh
./scripts/desktop-installer-smoke.sh target/node-desktop-package/uhorse-node-desktop-<version>-<target>
```

Windows 则使用：

```powershell
./scripts/package-node-desktop-windows-installer.ps1
```

验收标准是 archive、`.pkg` / installer 可生成，且宿主 API、静态资源与安装后 `bin + web` 布局 smoke 通过；当前仍不要求 `.app/.dmg`、签名、公证、`.msi` 或 Linux 原生安装器存在。

---

## 脚本说明

仓库根目录的以下脚本仍然存在：

- `install.sh`
- `quick-setup.sh`
- `start.sh`
- `stop.sh`

现在它们已经收口到当前 Hub-Node 主线：

- `install.sh`：编译 `uhorse-hub` / `uhorse-node` 并生成最小 `hub.toml` / `node.toml`
- `quick-setup.sh`：快速生成本地最小配置
- `start.sh` / `stop.sh`：仅管理本地 `uhorse-hub`

但如果你的目标是：

- 本地验证 Hub-Node 闭环
- 配置 DingTalk Stream 或 Node Desktop 通知镜像到钉钉
- 配置 LLM / 自定义模型服务商
- 部署 Hub 到服务器、Node 到本机

请优先使用本文档中的 `uhorse-hub` / `uhorse-node` 命令。

---

## 常见问题

### Rust 版本过低

```bash
rustc --version
rustup update
```

### OpenSSL / pkg-config 缺失

**macOS**

```bash
brew install openssl pkg-config
```

**Ubuntu / Debian**

```bash
sudo apt-get update
sudo apt-get install -y libssl-dev pkg-config
```

### 只编译了一个二进制

请确认命令包含两个 package：

```bash
cargo build --release -p uhorse-hub -p uhorse-node
```

### Node 无法连接 Hub

先检查：

- `hub.toml` 的监听端口
- `node.toml` 的 `connection.hub_url`
- 是否使用了 `/ws` 路径

例如：

```toml
[connection]
hub_url = "ws://127.0.0.1:8765/ws"
```

---

## 下一步

- [README.md](README.md)：项目总览
- [CONFIG.md](CONFIG.md)：配置结构与示例
- [LOCAL_SETUP.md](LOCAL_SETUP.md)：本地 Hub-Node 启动指南
- [TESTING.md](TESTING.md)：编译、测试与闭环验证
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)：v4 Hub-Node 部署说明
