<p align="center">
  <a href="README-en.md">English</a> | <strong>简体中文</strong>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>v4.6.0 Hub-Node 主线：企业协作入口 + 本地执行节点</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.6.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-released-green" alt="Status">
</p>

<p align="center">
  <a href="#项目定位">项目定位</a> •
  <a href="#当前能力">当前能力</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#架构">架构</a> •
  <a href="#文档索引">文档索引</a>
</p>

---

## 项目定位

uHorse 是一个面向企业协作场景的 Hub-Node 执行系统：Hub 负责接入企业消息通道、维护会话、调度任务与处理审批；Node / Node Desktop 负责在用户本地工作区执行受控命令，并把结果回传给 Hub 与原始协作会话。

当前正式主线是 **v4.6.0 Hub-Node 架构**，主交付物是：

- `uhorse-hub`：云端中枢，负责 Web API、WebSocket、Node 注册、任务调度、审批、通道接入与结果回包。
- `uhorse-node`：本地 Node CLI，负责连接 Hub、接收任务、执行命令、申请审批与回传结果。
- `uhorse-node-desktop`：当前推荐的桌面 Node 形态，提供本地宿主 API、Web UI、配置管理、连接诊断、恢复动作与运行时绑定。

当前仓库不再把旧单体 `uhorse` 二进制、旧 `/health/live` / `/health/ready` / `/api/v1/auth/*` / `/api/v1/messages`，或旧独立 Agent 平台作为当前主线入口。

## 当前能力

| 模块 | 状态 | 说明 |
|------|------|------|
| Hub-Node 主链路 | 已完成 | Hub HTTP API、WebSocket、Node 注册、心跳、任务下发与结果回传已闭合 |
| Node 鉴权 | 已完成 | 支持 JWT 引导，token 与注册 `node_id` 不一致会被拒绝 |
| 任务调度 | 已完成 | 文件、shell、受控 browser 命令进入统一调度链路 |
| 审批闭环 | 已完成 | `ApprovalRequest -> /api/approvals -> ApprovalResponse -> TaskResult` |
| Runtime session | 已完成 | `/api/v1/sessions*` 暴露 namespace、上下文链、可见性链与协作工作空间 |
| Serialized lane | 已完成 | 同一会话通过 `run_serialized(...)` 保序执行，continuation 绑定保留 `ReplyContext` |
| DingTalk | 主生产路径 | Stream 入站、自然语言规划、原消息 reaction、AI Card / transient handle 与结果回传均接入主链路 |
| Feishu | 最小第二样本 | 支持 webhook challenge、message event prepared inbound、`ReplyContext` 原消息回包 |
| WeWork | 配置 / 初始化样本 | 支持统一配置初始化，尚未进入 Hub prepared inbound 主线 |
| Generic reply-context | 已完成 | 声明 `REPLY_CONTEXT` 的通道可走 `Channel::reply_via_context(...)` generic dispatcher |
| Node Desktop | 已完成 | 支持本地配置、状态展示、连接诊断、恢复动作、DingTalk pairing 与重启提示 |
| 桌面交付 | 已完成 | 交付边界是 `bin + web` archive、macOS `.pkg`、Windows installer |
| Skill 在线安装 | 已完成 | 支持 HTTP 安装、DingTalk 薄入口、`.zip` / `.tar.gz`、`skill.yaml` 自动生成与 Python `.venv` 依赖安装 |
| 可观测性 | 已完成 | `GET /api/health`、`GET /metrics`、审批 wait / resume metrics、危险操作与 restore 审计事件 |

## 快速开始

### 1. 编译主线二进制

```bash
git clone https://github.com/kts-kris/uHorse
cd uHorse
cargo build --release -p uhorse-hub -p uhorse-node -p uhorse-node-desktop
```

主线产物：

- `target/release/uhorse-hub`
- `target/release/uhorse-node`
- `target/release/uhorse-node-desktop`

也可以从 GitHub Release / nightly 获取 `uhorse-hub` archive，以及 `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer。

### 2. 生成配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

如果只验证 Hub ↔ Node 基础闭环，可以使用最小配置。

`hub.toml`：

```toml
hub_id = "local-hub"
bind_address = "127.0.0.1"
port = 8765
max_nodes = 10
heartbeat_timeout_secs = 30
task_timeout_secs = 60
max_retries = 3
```

`node.toml`：

```toml
name = "local-node"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
```

需要启用 DingTalk、Feishu、WeWork、LLM、JWT 或审批时，请使用统一配置结构，详见 [CONFIG.md](CONFIG.md)。

### 3. 启动 Hub 与 Node

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
./target/release/uhorse-node --config node.toml --log-level info
```

基础验证：

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

提交一个最小任务：

```bash
curl -X POST http://127.0.0.1:8765/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "command": {
      "type": "file",
      "action": "exists",
      "path": "/tmp/demo.txt"
    },
    "user_id": "api-user",
    "session_id": "api-session",
    "channel": "api"
  }'
```

查询任务：

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

### 4. 默认回归入口

```bash
make test-quick
make skill-install-smoke
cargo test -p uhorse-hub
cargo test -p uhorse-channel
cargo test -p uhorse-config
```

重点通道与 continuation 回归：

```bash
cargo test -p uhorse-hub test_dispatch_reply_via_context_uses_generic_channel_reply_path -- --nocapture
cargo test -p uhorse-hub test_prepare_feishu_inbound_and_submit_turn_dispatches_assignment -- --nocapture
cargo test -p uhorse-hub session_key_from_reply_context -- --nocapture
```

## 架构

```text
企业协作入口
DingTalk Stream / Feishu Webhook / HTTP API
        │
        ▼
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  Web API: /api/health /metrics /api/*        │
│  WebSocket: /ws                              │
│  Channel adapters / reply dispatcher         │
│  Session runtime / serialized lane           │
│  Task scheduler / approval / audit           │
└──────────────────────────────────────────────┘
        │
        │ HubToNode / NodeToHub WebSocket
        ▼
┌──────────────────────────────────────────────┐
│             uhorse-node-runtime              │
│  Connection loop / reconnect                 │
│  Workspace guard / permission manager        │
│  File / shell / browser command executor     │
│  TaskResult / ApprovalRequest                │
└──────────────────────────────────────────────┘
        │
        ▼
┌──────────────────────────────────────────────┐
│           Local workspace / desktop host      │
└──────────────────────────────────────────────┘
```

### 关键源码入口

| 路径 | 说明 |
|------|------|
| `crates/uhorse-hub/src/main.rs` | Hub 启动、统一配置加载、通道初始化 |
| `crates/uhorse-hub/src/web/mod.rs` | Hub Web API、DingTalk / Feishu 入站、reply-context 回包、任务规划 |
| `crates/uhorse-hub/src/session_runtime.rs` | Runtime session、transcript、continuation 绑定与 serialized lane |
| `crates/uhorse-hub/src/task_scheduler.rs` | 任务调度与任务状态 |
| `crates/uhorse-channel/src/` | DingTalk、Feishu、WeWork 等通道实现与 registry |
| `crates/uhorse-core/src/traits.rs` | `Channel` trait 与 `reply_via_context` 最小契约 |
| `crates/uhorse-node/src/main.rs` | Node CLI 入口 |
| `crates/uhorse-node-runtime/src/` | Node 连接、权限、工作区与命令执行 |
| `crates/uhorse-node-desktop/src/` | Node Desktop 本地宿主 API |
| `apps/node-desktop-web/` | Node Desktop Web UI |

## 通道边界

### DingTalk

DingTalk 是当前生产主路径：

- 推荐使用 Stream 模式接收入站消息。
- 自然语言消息可进入 Hub → Node 任务链路。
- 处理中状态优先使用 AI Card；未命中时优先在原消息上贴 `🤔思考中` reaction，并在任务完成、失败或取消后 best-effort recall / clear。
- Skill 安装薄入口支持 `安装技能 <package> <download_url> [version]`，也支持上传 `.zip` 后追发“帮我安装这个技能”。

### Feishu

Feishu 是 multi-channel reply-context 抽象的最小第二样本：

- `GET /api/v1/channels/feishu/webhook` 返回 readiness 文本。
- `POST /api/v1/channels/feishu/webhook` 支持 challenge 响应。
- message event 可被预处理为 `PreparedInboundTurn` 并进入 Hub 调度主线。
- 普通回包走 `ReplyContext` + `Channel::reply_via_context(...)`，优先使用原始 `message_id` 调用 Feishu reply API。

### WeWork 与其他通道

WeWork 当前具备统一配置与初始化样本，但尚未进入 Hub prepared inbound 主线。Telegram、Slack、Discord、WhatsApp 等模块保留，不是当前 `v4.6.0` Hub 主线验证重点。

## Node Desktop 交付边界

Node Desktop 当前定位是完整本地 Node 客户端，而不是单纯包装二进制。它负责：

- 本地配置读写与重启提示。
- Hub 连接状态、生命周期、认证前提与工作区校验。
- 连接诊断、最小恢复动作与最近日志摘要。
- DingTalk pairing 驱动的运行时绑定。
- 本地通知展示与可选 DingTalk 镜像。

当前正式交付物是：

- `bin + web` archive
- macOS `.pkg`
- Windows installer

当前非目标：原生 `.app/.dmg`、签名、公证、`.msi`、Linux 原生安装器或拖拽安装体验。

## 工作区结构

```text
crates/
├── uhorse-hub/             # 云端中枢
├── uhorse-node/            # Node CLI 二进制入口
├── uhorse-node-runtime/    # Node 实际运行时
├── uhorse-node-desktop/    # Node Desktop 本地宿主
├── uhorse-protocol/        # Hub-Node 协议
├── uhorse-channel/         # 企业通道实现
├── uhorse-config/          # 统一配置模型
├── uhorse-core/            # 核心类型与 trait
├── uhorse-llm/             # LLM 客户端
└── ...

apps/
└── node-desktop-web/       # Node Desktop Web UI

scripts/                   # 打包、smoke、release 辅助脚本
docs/                      # 架构与设计文档
deployments/               # 部署说明
```

## 当前非目标

- 不恢复旧单体 Agent 平台作为主交付物。
- 不把 legacy `uhorse` 二进制作为当前默认运行路径。
- 不把旧 `/health/live`、`/health/ready`、`/api/v1/auth/*`、`/api/v1/messages` 当作当前主线 API。
- 不把 DingTalk AI Card / reaction / transient handle 强行泛化为跨通道处理中句柄 trait。
- 不一次性把 WeWork、Telegram、Slack、Discord、WhatsApp 接入 Hub prepared inbound 主线。
- 不把 Node Desktop 当前交付边界扩展为原生 `.app/.dmg`、签名、公证、`.msi` 或 Linux 原生安装器。

## 文档索引

| 文档 | 说明 |
|------|------|
| [README-en.md](README-en.md) | English README |
| [INSTALL.md](INSTALL.md) | 安装、构建、桌面包与 smoke 验证 |
| [CONFIG.md](CONFIG.md) | Hub / Node 统一配置、DingTalk / Feishu / WeWork 配置样例 |
| [CHANNELS.md](CHANNELS.md) | 通道现状、DingTalk 主路径、Feishu 最小样本与边界 |
| [API.md](API.md) | 当前已实现 API 表面 |
| [SKILLS.md](SKILLS.md) | Skill 包结构、在线安装与 Python Skill 兼容行为 |
| [TESTING.md](TESTING.md) | 测试入口、回归顺序与手工验证路径 |
| [PROGRESS.md](PROGRESS.md) | 当前实施进度与高层状态 |
| [CHANGELOG.md](CHANGELOG.md) | 版本变更记录 |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | 当前发布说明 |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | 本地双进程联调、JWT、审批与重连验证 |
| [scripts/README.md](scripts/README.md) | 脚本、打包与 smoke 说明 |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4 Hub-Node 部署路径 |
| [docs/architecture/v4.0-architecture.md](docs/architecture/v4.0-architecture.md) | v4 架构说明 |

## 许可证

双许可：[MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)
