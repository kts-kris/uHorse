# uHorse 主线脚本说明

本目录脚本围绕 **当前 `v4.1.3` Hub + Node 主线** 组织，不再默认验证旧单体 `uhorse`、旧 `/health/live`、旧 `/health/ready`。

## 可用脚本

### `dev.sh`

热重载启动 `uhorse-hub`：

```bash
./scripts/dev.sh
```

默认行为：

- 启动 `cargo watch`
- 运行 `uhorse-hub --host 127.0.0.1 --port 8765`
- 健康检查地址为 `http://127.0.0.1:8765/api/health`

### `start.sh`

前台启动当前主线 Hub：

```bash
./scripts/start.sh
```

该脚本等价于仓库根目录 `./run.sh`。

### `quick-test.sh`

快速验证当前主线关键链路：

```bash
./scripts/quick-test.sh
```

覆盖内容：

- `uhorse-hub` / `uhorse-node` release 编译
- 真实 `test_local_hub_node_roundtrip_file_exists`
- `uhorse-node check --workspace .`
- Hub Docker 构建
- Docker 内 `GET /api/health` 与 `GET /api/nodes` smoke

### `test.sh`

执行更完整的主线回归：

```bash
./scripts/test.sh
```

覆盖内容：

- `uhorse-node-runtime` 包级测试
- `uhorse-hub` 包级测试
- roundtrip 回归
- JWT `node_id` 不匹配拒绝回归
- Node workspace 检查
- Hub Docker smoke

### `package-node-desktop.sh`

打包 Node Desktop 宿主与前端静态资源。

这是当前 `v4.1.3` Node Desktop 交付链路里的 archive 打包入口：

```bash
./scripts/package-node-desktop.sh
```

默认会：

- 构建 `apps/node-desktop-web`
- 编译 `uhorse-node-desktop`
- 生成 `target/node-desktop-package/uhorse-node-desktop-<version>-<target>/`
- 输出对应 `.tar.gz` 或 `.zip` 压缩包

### `package-node-desktop-macos-pkg.sh`

基于现有 Node Desktop payload 生成 macOS `.pkg`。

```bash
./scripts/package-node-desktop-macos-pkg.sh
```

前提：

- 先执行 `./scripts/package-node-desktop.sh`
- 当前 target 为 `*apple-darwin*`
- 本机存在 `pkgbuild`

默认输出：

- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>.pkg`

安装内容保持现有 `bin + web` 布局，并额外附带 `uHorse Node Desktop.command` launcher。

### `package-node-desktop-windows-installer.ps1`

基于现有 Node Desktop payload 生成 Windows installer `.exe`。

```powershell
./scripts/package-node-desktop-windows-installer.ps1
```

前提：

- 先执行 `./scripts/package-node-desktop.sh`
- 当前 target 为 Windows
- 本机存在 `makensis.exe`

默认输出：

- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>-installer.exe`

安装内容保持现有 `bin + web` 布局，并额外附带 `start-node-desktop.cmd` launcher。

### `desktop-smoke.sh`

运行 Node Desktop 宿主 API + 静态资源 smoke。

这是当前 `v4.1.3` archive 验收链路里的运行验证入口，用来确认 archive 解包后的宿主与前端资源可正常工作：

```bash
./scripts/desktop-smoke.sh
```

覆盖内容：

- `apps/node-desktop-web` 构建
- `uhorse-node-desktop` release 编译
- `GET /api/settings/defaults`
- `GET /api/settings/capabilities`
- `GET /api/workspace/status`
- `GET /api/runtime/status`
- `GET /api/versioning/summary`
- `/` 与前端路由回退静态资源可访问

### `desktop-installer-smoke.sh`

运行安装后目录的 Node Desktop 宿主 API + 静态资源 smoke。

```bash
./scripts/desktop-installer-smoke.sh <install-root>
```

### `desktop-installer-smoke.ps1`

运行 Windows 安装后目录的 Node Desktop 宿主 API + 静态资源 smoke。

```powershell
./scripts/desktop-installer-smoke.ps1 -InstallRoot <install-root>
```

覆盖内容：

- 安装根下 `bin/uhorse-node-desktop` / `bin/uhorse-node-desktop.exe`
- 安装根下 `web/index.html` 与 `web/assets`
- `GET /api/settings/defaults`
- `GET /api/settings/capabilities`
- `GET /api/workspace/status`
- `GET /api/runtime/status`
- `GET /api/versioning/summary`
- `/` 与前端路由回退静态资源可访问

当前 release / nightly 会继续保留 archive，并额外产出：

- macOS `.pkg`
- Windows installer `.exe`

当前仍不包含 `.app/.dmg`、签名、公证、`.msi` 或 Linux 原生安装器。

## 推荐搭配

```bash
make build
make start
make node-run
make test-quick
make test-full
make desktop-package
make desktop-package-macos
make desktop-package-windows
make desktop-smoke
make desktop-installer-smoke INSTALL_ROOT=target/node-desktop-package/uhorse-node-desktop-<version>-<target>
```

## 参考文档

- `LOCAL_SETUP.md`：Hub / Node 本地启动与联调
- `TESTING.md`：测试矩阵与回归命令
- `INSTALL.md`：安装与最小闭环入口
