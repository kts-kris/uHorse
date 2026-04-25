<p align="center">
  <a href="README-en.md">English</a> | <strong>简体中文</strong>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>uHorse v4.6.0 当前正式发布线</strong>
</p>

<p align="center">
  <em>Hub 负责调度与通道接入，Node 负责本地执行与结果回传；当前仓库 HEAD 已收口为正式发布的 `v4.6.0`，主交付物为 `uhorse-hub` 与 `uhorse-node-desktop`。</em>
</p>

<p align="center">
  <a href="#概述">概述</a> •
  <a href="#当前状态">当前状态</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#架构">架构</a> •
  <a href="#文档索引">文档索引</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.6.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-released-green" alt="Status">
</p>

---

## 概述

uHorse 当前对外正式发布口径是 **v4.6.0 Hub-Node 主线**。

核心组件与主交付物：

- `uhorse-hub`：云端中枢，负责 Node 接入、任务调度、Web API、审批接口，以及 DingTalk Stream 消息入口。
- `uhorse-node-runtime`：Node 的实际运行时实现，包括连接循环、工作区保护、权限管理、重连、浏览器执行与任务执行。
- `uhorse-node-desktop`：当前推荐的本地 Node 桌面形态，交付边界为 `bin/ + web/` archive，并提供 macOS `.pkg` 与 Windows installer。
- `uhorse-protocol`：Hub 和 Node 之间的协议定义，包括 `TaskAssignment`、`TaskResult`、`ApprovalRequest`、`ApprovalResponse` 等消息。
- `uhorse-config`：Hub 统一配置模型，承载 `server`、`channels`、`security`、`llm` 等配置段。

`v4.6.0` 已落地并对外可见的能力包括：

- DingTalk 自然语言请求可进入 Hub → Node 链路，并在受控场景下规划为 `BrowserCommand`。
- Hub 已对浏览器目标执行本地安全校验，拒绝 `file://`、localhost、私网地址和其他越界目标。
- Node Desktop 与 runtime 已支持浏览器能力路由，浏览器任务会优先调度到声明 `CommandType::Browser` 的节点；对于“打开网页”这类 DingTalk 指令，主线契约会规划为 `BrowserCommand::OpenSystem`，以宿主机系统浏览器语义执行。
- DingTalk 现在会优先在用户原消息上贴 `🤔思考中` reaction，并在任务完成、失败或取消后自动 recall；如果 reaction attach 失败，则会 best-effort 回退到现有处理中路径。
- 通道抽象已补齐最小 `ReplyContext` 回包契约：Hub 可对声明 `REPLY_CONTEXT` 能力的通道走 generic reply path；DingTalk 的 AI Card / reaction / transient handle 生命周期仍保留在专用 adapter 内。
- Feishu 已作为第二样本接入最小 Hub 主线：支持 webhook challenge、message event 预处理、prepared inbound 提交，以及基于原消息 `message_id` 的 reply-context 回包。
- `memory / agent / skill` 已支持 `global / tenant / enterprise / department / role / user / session` 分层共享链；`memory_context_chain` 从共享读到私有，`visibility_chain` 从私有回退到共享。
- 任务上下文与 runtime session 已显式区分稳定 `execution_workspace_id` 和 Hub 侧逻辑 `collaboration_workspace_id` / `CollaborationWorkspace`；前者决定真实执行边界，后者仅承载协作上下文与默认绑定。
- runtime API 与 Web UI 已支持 `source_layer`、`source_scope` 的来源感知展示与按来源详情查询。
- Node Desktop 当前交付边界是 `bin/ + web/` archive、macOS `.pkg`、Windows installer、对应 smoke 与 GitHub release / nightly artifacts，而不是原生 `.app/.dmg`、签名、公证、`.msi` 或 Linux 原生安装器。

当前文档以 **仓库里已实现并验证的行为** 为准，不再把旧版 `/health/live`、`/health/ready`、`/api/v1/auth/*`、`/api/v1/messages` 当作当前主线，也不把 `v4.6.0` 写成旧单体 Agent 平台回归。

## 当前状态

| 能力 | 状态 | 说明 |
|------|------|------|
| Hub 本机启动 | ✅ | 当前实际观测入口为 `GET /api/health` 与 `GET /metrics` |
| Node 本机启动 | ✅ | `uhorse-node` 可加载 `node.toml` 并连接 `ws://.../ws` |
| Node JWT 引导 | ✅ | `POST /api/node-auth/token` 可在启用 `[security].jwt_secret` 时签发 token |
| Hub → Node 任务下发 | ✅ | `POST /api/tasks` 提交后进入调度器 |
| Node → Hub 结果回传 | ✅ | Node 回传完整 `NodeToHub::TaskResult` |
| 审批闭环 | ✅ | `ApprovalRequest -> /api/approvals -> ApprovalResponse -> TaskResult` |
| Hub 重启后 Node 重连 | ✅ | Node 具备自动重连与重新注册能力 |
| 多用户 `memory / agent / skill` 分层作用域 | ✅ | 当前 runtime 已按 `global / tenant / enterprise / department / role / user / session` 组织共享与隔离边界 |
| 运行时 session / 协作工作空间 API | ✅ | `/api/v1/sessions*` 已返回 `namespace`、`memory_context_chain`、`visibility_chain` 与 `collaboration_workspace` |
| source-aware runtime / UI | ✅ | Skills、Settings 等页面已展示 `source_layer`、`source_scope`，同名多来源资源可区分 |
| Node Desktop 打包与 smoke | ✅ | 当前交付为 `bin + web` archive + macOS `.pkg` + Windows installer，CI / release / nightly 均产出对应 artifact，不包含 `.app/.dmg`、`.msi` 或 Linux 原生安装器 |
| 本地真实集成测试 | ✅ | `test_local_hub_node_roundtrip_file_exists` 与 `test_local_hub_node_roundtrip_file_write` 已覆盖真实 Hub + Node + WebSocket 闭环 |
| 鉴权拒绝路径 | ✅ | `test_local_hub_rejects_node_with_mismatched_auth_token` 已覆盖 token 与注册 `node_id` 不一致场景 |
| DingTalk Stream 接入 | ✅ | 当前推荐模式为 Stream；Node Desktop 可通过 pairing 发起运行时绑定，`channels.dingtalk.notification_bindings` 仅作为兼容 seed/fallback |
| Generic reply-context 回包 | ✅ | `ReplyContext` + `Channel::reply_via_context` 已接入 Hub generic dispatcher，DingTalk 高级处理中句柄仍由专用 adapter 保留 |
| Feishu 最小入站样本 | ✅ | 支持 `/api/v1/channels/feishu/webhook` challenge / message event 预处理，并可进入 prepared inbound 主线 |
| DingTalk 浏览器规划链路 | ✅ | Hub 已允许受控 `BrowserCommand`，并可把浏览器任务调度到具备 `CommandType::Browser` 的节点 |
| Agent Browser Skill 安装自动化回归 | ✅ | `test_agent_browser_natural_language_install_flow_returns_chinese_hint` 已覆盖自然语言安装、SkillHub 安装与中文提示 |
| 在线 Skill 安装兼容性 | ✅ | `POST /api/v1/skills/install` 与 DingTalk 安装薄入口已支持 `.zip` / `.tar.gz`，并兼容 `skill.yaml` Python Skill 自动生成 `skill.toml`、`.venv` 依赖安装 |

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

如需直接获取主流平台包，也可以使用 GitHub Release / nightly 中的 `uhorse-hub` archive，以及 `uhorse-node-desktop` archive / macOS `.pkg` / Windows installer。

### 2. 生成默认配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 3. 最小本地闭环

如果你只想先验证 Hub ↔ Node 基础链路，可使用最小配置：

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

启动：

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
./target/release/uhorse-node --config node.toml --log-level info
```

验证：

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 4. 默认快速回归入口

当前推荐的默认快速回归入口：

```bash
make test-quick
make skill-install-smoke
```

其中：

- `make test-quick` 当前会默认执行 release 编译、真实 Hub-Node roundtrip、Agent Browser Skill 安装自动化回归、Node workspace 检查和 Hub Docker smoke。
- `make skill-install-smoke` 会单独运行 `test_agent_browser_natural_language_install_flow_returns_chinese_hint`，验证“帮我安装 Agent Browser 技能”自然语言安装、SkillHub 安装与中文提示链路。

### 5. 启用鉴权与审批

如果你要验证 Node JWT、审批接口或与当前 Hub-Node 主线完全一致的链路，请让 Hub 使用统一配置并设置 `[security].jwt_secret`：

```toml
[server]
host = "127.0.0.1"
port = 8765

[security]
jwt_secret = "replace-with-random-secret"
token_expiry = 86400
refresh_token_expiry = 2592000
pairing_expiry = 300
approval_enabled = true
pairing_enabled = true
```

启动 Hub 后，为固定 `node_id` 签发 token：

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

把返回的 `access_token` 写入 `node.toml`：

```toml
name = "office-node-01"
node_id = "office-node-01"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
auth_token = "<access_token>"
```

如果你要把 Node Desktop 本地通知镜像到钉钉，还需要同时满足两侧配置：

- Node Desktop 本地开启 `notifications_enabled`
- 如需在通知中展示更详细内容，再开启 `show_notification_details`
- 若要把本地通知额外同步到钉钉，再开启 `mirror_notifications_to_dingtalk`
- 在 Node Desktop 中发起 pairing，并在 DingTalk 中使用**当前页面最新显示**的绑定码确认运行时绑定；如果重新生成过绑定码，请只发送最新那一条
- DingTalk Stream 入站与 Web 路径现在都会优先尝试处理 pairing 命令，绑定码消息不会再误走普通任务规划链路
- `channels.dingtalk.notification_bindings` 仅在需要兼容 seed/fallback 时配置
- 若当前运行中的 Node 与新保存配置不一致，Settings / Dashboard 会显示“需重启生效”，重启后才会切换到新的工作区与运行时配置

### 5. 在线安装 Skill（HTTP / DingTalk）

当前在线安装链路已补齐以下兼容性：

- `POST /api/v1/skills/install` 支持 `.tar.gz` 与 `.zip` 安装包
- DingTalk 除 `安装技能 <package> <download_url> [version]` 外，也支持先上传一个 `.zip` 技能包，再跟一句“帮我安装这个技能”
- 对仅提供 `skill.yaml` + `src/main.py` / `main.py` 的 Python Skill，Hub 会自动生成 `skill.toml`
- 如果安装包带有 `requirements.txt`，Hub 会自动创建 `.venv` 并安装依赖，再刷新运行时 Skill registry

相关接口与边界说明见 [SKILLS.md](SKILLS.md)、[API.md](API.md)、[CHANNELS.md](CHANNELS.md)。

### 6. 提交一个最小任务

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

查询任务状态请使用：

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

> 注意：当前 `GET /api/tasks` 仍是占位实现；真实状态以 `GET /api/tasks/:task_id` 为准。

## 架构

```text
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  • Web API: /api/health /metrics /api/*     │
│  • WebSocket: /ws                            │
│  • Task Scheduler                            │
│  • Approval API                              │
│  • DingTalk Stream                           │
└──────────────────────────────────────────────┘
                      │
                      │ WebSocket
                      ▼
┌──────────────────────────────────────────────┐
│             uhorse-node-runtime              │
│  • Connection loop / reconnect               │
│  • Workspace protection                      │
│  • Permission manager                        │
│  • Command executor                          │
│  • TaskResult / ApprovalRequest              │
└──────────────────────────────────────────────┘
                      │
                      ▼
               ┌───────────────┐
               │  workspace    │
               └───────────────┘
```

### 当前关键源码入口

- Hub 启动与统一配置：`crates/uhorse-hub/src/main.rs`
- Hub Web API 与 DingTalk / Browser 规划校验：`crates/uhorse-hub/src/web/mod.rs`
- Hub WebSocket 鉴权与注册：`crates/uhorse-hub/src/web/ws.rs`
- Hub 调度器：`crates/uhorse-hub/src/task_scheduler.rs`
- Node 启动入口：`crates/uhorse-node/src/main.rs`
- Node 运行时：`crates/uhorse-node-runtime/src/node.rs`
- Node 浏览器执行与命令调度：`crates/uhorse-node-runtime/src/executor.rs`
- Node Desktop 桌面宿主：`crates/uhorse-node-desktop/src/main.rs`
- 本地集成测试：`crates/uhorse-hub/tests/integration_test.rs`

## 文档索引

| 文档 | 说明 |
|------|------|
| [CHANGELOG.md](CHANGELOG.md) | `v4.6.0` 发布事实、文档同步记录与当前非目标说明 |
| [INSTALL.md](INSTALL.md) | 当前 Hub-Node 安装路径与 Node Desktop archive / smoke 边界 |
| [API.md](API.md) | 当前已实现的 Hub-Node API 参考 |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | 本地双进程联调、JWT 引导、审批与重连回归 |
| [CONFIG.md](CONFIG.md) | 统一配置、legacy HubConfig、NodeConfig 与权限规则 |
| [CHANNELS.md](CHANNELS.md) | 通道现状、DingTalk Stream、浏览器规划链路与通知镜像说明 |
| [SKILLS.md](SKILLS.md) | 在线 Skill 安装、Skill 包结构与 Python Skill 兼容说明 |
| [scripts/README.md](scripts/README.md) | 主线脚本说明，包括 Node Desktop package / smoke 与 CI / release 对齐 |
| [TESTING.md](TESTING.md) | 包级测试、工作区测试、`make test-quick` / `make skill-install-smoke` 与手工回归顺序 |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | `v4.6.0` 发布说明与当前在线 Skill 安装兼容性补充 |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4 Hub-Node 部署路径 |
| [docs/architecture/v4.0-architecture.md](docs/architecture/v4.0-architecture.md) | v4 架构说明 |

## 工作区结构

```text
crates/
├── uhorse-hub/           # 云端中枢
├── uhorse-node/          # Node CLI 二进制入口
├── uhorse-node-runtime/  # Node 实际运行时
├── uhorse-protocol/      # Hub-Node 协议
├── uhorse-channel/       # 通道实现
├── uhorse-config/        # 统一配置模型
├── uhorse-llm/           # LLM 客户端
└── ...
```

## 许可证

双许可：[MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)
