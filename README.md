<p align="center">
  <a href="README-en.md">English</a> | <strong>简体中文</strong>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>v4.0 Hub-Node 分布式 AI 执行平台</strong>
</p>

<p align="center">
  <em>Hub 负责调度，Node 负责本地执行，DingTalk Stream 负责企业消息入口。</em>
</p>

<p align="center">
  <a href="#概述">概述</a> •
  <a href="#当前状态">当前状态</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#架构">架构</a> •
  <a href="#文档索引">文档索引</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-4.0.0--alpha.1-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.78%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-alpha-yellow" alt="Status">
</p>

---

## 概述

uHorse 当前仓库的主线是 **v4.0 Hub-Node 架构**：

- `uhorse-hub`：云端中枢，负责节点接入、任务调度、Web API、DingTalk 消息入口与结果回传。
- `uhorse-node`：本地执行节点，负责在受控工作空间内执行命令并回传结果。
- `uhorse-protocol`：Hub 和 Node 之间的消息协议。
- `uhorse-channel`：当前 Hub 运行时已接入 **DingTalk Stream 模式**。
- `uhorse-config`：统一配置结构，供 Hub 读取 DingTalk / LLM / 基础服务配置。

当前文档以 **仓库里已经实现并验证过的行为** 为准，不再沿用旧版单体 `uhorse` 的启动方式、旧健康检查路径或 `OPENCLAW_*` 环境变量说明。

## 当前状态

| 能力 | 状态 | 说明 |
|------|------|------|
| Hub 本机启动 | ✅ | `uhorse-hub` 可正常启动并提供 `/api/health`、`/api/nodes`、`/ws` |
| Node 本机启动 | ✅ | `uhorse-node` 可加载 `node.toml` 并连接 Hub |
| Hub → Node 任务下发 | ✅ | 任务提交后会触发调度 |
| Node → Hub 结果回传 | ✅ | Node 会发送完整 `NodeToHub::TaskResult` |
| 本地闭环验证 | ✅ | 已有真实集成测试 `test_local_hub_node_roundtrip_file_exists` |
| DingTalk Stream 接入 | ✅ | 当前方向为 Stream 模式，无需公网 IP 才能建立消息流 |
| DingTalk 真实企业凭据联调 | ⏳ | 代码链路已接通，最终联调仍依赖真实企业配置 |

## 快速开始

### 1. 编译二进制

```bash
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

cargo build --release -p uhorse-hub -p uhorse-node
```

编译结果：

- `target/release/uhorse-hub`
- `target/release/uhorse-node`

### 2. 生成默认配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

如果你只想先做最小本地闭环，可以直接使用下面这组最小配置。

### 3. 最小本地闭环配置

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

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
auth_token = ""
```

### 4. 启动 Hub 和 Node

终端 1：

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

终端 2：

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

### 5. 验证连接

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/api/nodes
```

如果 `api/nodes` 返回在线节点列表，说明 Hub 和 Node 已完成连接。

### 6. 跑真实本地 roundtrip 集成测试

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

这条测试会真实启动 Hub、WebSocket 服务和 Node，验证文件命令从 Hub 下发到 Node，再由 Node 把结果回传给 Hub。

## DingTalk Stream 模式

当前 `uhorse-hub` 主运行时已接入 DingTalk：

- 推荐模式：**Stream 模式**
- 优点：无需公网 IP、无需额外 webhook 暴露即可接收入站消息
- 当前支持的管理命令白名单：`list` / `ls`、`search`、`read` / `cat`、`info`、`exists`
- Hub 会把 Node 执行结果按会话路由回发到 DingTalk

要启用 DingTalk，请使用统一配置文件，见 [CONFIG.md](CONFIG.md) 和 [CHANNELS.md](CHANNELS.md)。

## 架构

```text
┌──────────────────────────────────────────────┐
│                  uhorse-hub                  │
│  • Web API: /api/health /api/nodes /api/*   │
│  • WebSocket: /ws                            │
│  • Task Scheduler                            │
│  • DingTalk Stream / result reply            │
└──────────────────────────────────────────────┘
                      │
                      │ WebSocket
                      ▼
┌──────────────────────────────────────────────┐
│                 uhorse-node                  │
│  • Workspace                                 │
│  • Permission Manager                        │
│  • Command Executor                          │
│  • TaskResult 回传                           │
└──────────────────────────────────────────────┘
```

### 当前关键源码入口

- Hub 启动与统一配置：`crates/uhorse-hub/src/main.rs`
- Hub Web API 与 DingTalk 路由：`crates/uhorse-hub/src/web/mod.rs`
- Hub 核心调度：`crates/uhorse-hub/src/hub.rs`
- Node 启动入口：`crates/uhorse-node/src/main.rs`
- Node 执行与结果回传：`crates/uhorse-node/src/node.rs`
- 本地闭环测试：`crates/uhorse-hub/tests/integration_test.rs`

## 文档索引

| 文档 | 说明 |
|------|------|
| [INSTALL.md](INSTALL.md) | 安装与二进制构建 |
| [LOCAL_SETUP.md](LOCAL_SETUP.md) | 本地 Hub-Node 开发与启动 |
| [CONFIG.md](CONFIG.md) | 真实配置结构与示例 |
| [CHANNELS.md](CHANNELS.md) | 当前通道现状，重点是 DingTalk Stream |
| [TESTING.md](TESTING.md) | 编译、测试与本地闭环验证 |
| [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md) | v4.0 Hub-Node 部署指南 |
| [deployments/DEPLOYMENT.md](deployments/DEPLOYMENT.md) | 部署总览与迁移说明 |

## 工作区结构

```text
crates/
├── uhorse-hub/        # 云端中枢
├── uhorse-node/       # 本地节点
├── uhorse-protocol/   # Hub-Node 通信协议
├── uhorse-channel/    # 通道实现（当前 Hub 运行时重点为 DingTalk）
├── uhorse-config/     # 统一配置
├── uhorse-llm/        # LLM 客户端
└── ...                # 其他 3.x / 4.0 相关模块
```

## 许可证

双许可：[MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)
