# uHorse 实施进度

本文档记录当前仓库主线的高层实施状态。更细的接口、配置、通道和测试事实分别以 `API.md`、`CONFIG.md`、`CHANNELS.md`、`TESTING.md` 为准。

## 当前版本线

- 当前正式主线：`v4.6.0` Hub-Node 架构
- 主交付物：`uhorse-hub`、`uhorse-node`、`uhorse-node-desktop`
- 当前推荐本地闭环：Hub 负责调度与通道接入，Node 负责本地执行与结果回传
- 当前推荐桌面形态：Node Desktop `bin + web` archive、macOS `.pkg`、Windows installer

## 已完成能力

### Hub-Node 主链路

- [x] Hub HTTP API 与 WebSocket 接入
- [x] Node 注册、心跳、任务下发与结果回传
- [x] Node JWT 引导与 token / 注册 `node_id` 一致性校验
- [x] Hub 重启后 Node 自动重连与重新注册
- [x] `execution_workspace_id` 与 `collaboration_workspace_id` / `CollaborationWorkspace` 分离
- [x] 文件、shell、受控 browser 命令进入统一调度链路

### Runtime session / continuation

- [x] Runtime session API：`/api/v1/sessions*`
- [x] runtime namespace、`memory_context_chain`、`visibility_chain`
- [x] task continuation binding 保存 `ReplyContext`
- [x] serialized session lane 保障同一会话顺序执行
- [x] continuation fallback 可从 `ReplyContext` sender / team metadata 恢复通道 session key

### 通道主线

- [x] DingTalk Stream 作为当前生产主路径
- [x] DingTalk 自然语言规划进入 Hub → Node 任务链路
- [x] DingTalk 原消息 `🤔思考中` reaction attach / recall
- [x] DingTalk AI Card / reaction / transient handle 生命周期保留在专用 adapter 内
- [x] `Channel::reply_via_context(...)` 最小 generic 回包契约
- [x] Hub generic reply dispatcher 支持声明 `REPLY_CONTEXT` 的通道
- [x] Feishu 作为第二样本支持 webhook challenge、message event prepared inbound 与 reply-context 回包
- [x] WeWork 具备统一配置与初始化样本

### Node Desktop

- [x] 本地宿主 API 与 Web UI
- [x] Settings / Dashboard 展示运行状态、配置与重启提示
- [x] Node Desktop pairing 驱动的 DingTalk 运行时绑定
- [x] macOS `.pkg` 与 Windows installer 打包脚本
- [x] archive / installer smoke 验证脚本

### Skill / Agent / Memory

- [x] `memory / agent / skill` 支持 `global / tenant / enterprise / department / role / user / session` 分层链
- [x] source-aware runtime / UI：`source_layer`、`source_scope`
- [x] HTTP 在线 Skill 安装与 refresh API
- [x] DingTalk Skill 安装薄入口
- [x] `.zip` / `.tar.gz` 安装包兼容
- [x] `skill.yaml` Python Skill 自动生成 `skill.toml`
- [x] `requirements.txt` 触发 `.venv` 依赖安装

### 可观测性、审批与审计

- [x] `GET /api/health`
- [x] `GET /metrics`
- [x] approval request / approve / reject 闭环
- [x] approval wait / resume transcript 与 metrics
- [x] dangerous git deny 审计事件
- [x] workspace checkpoint / restore 与 backup restore 生命周期审计事件

## 当前验证入口

```bash
make test-quick
make skill-install-smoke
cargo test -p uhorse-hub
cargo test -p uhorse-channel
cargo test -p uhorse-config
cargo test -p uhorse-hub test_prepare_feishu_inbound_and_submit_turn_dispatches_assignment -- --nocapture
cargo test -p uhorse-hub session_key_from_reply_context -- --nocapture
```

## 当前非目标

- 不恢复旧单体 Agent 平台作为主交付物。
- 不把 legacy `uhorse` 二进制作为当前默认运行路径。
- 不把旧 `/health/live`、`/health/ready`、`/api/v1/auth/*`、`/api/v1/messages` 当作当前主线 API。
- 不在当前阶段把 DingTalk AI Card / reaction / transient handle 强行泛化为跨通道处理中句柄 trait。
- 不把 WeWork、Telegram、Slack、Discord、WhatsApp 一次性接入 Hub prepared inbound 主线。
- 不把 Node Desktop 当前交付边界扩展为原生 `.app/.dmg`、签名、公证、`.msi` 或 Linux 原生安装器。

## 文档索引

- [README.md](README.md)：项目总览
- [INSTALL.md](INSTALL.md)：安装与交付路径
- [CONFIG.md](CONFIG.md)：配置结构
- [CHANNELS.md](CHANNELS.md)：通道现状与 DingTalk / Feishu 边界
- [API.md](API.md)：当前 API 表面
- [TESTING.md](TESTING.md)：测试与回归入口
- [CHANGELOG.md](CHANGELOG.md)：版本变更记录

**最后更新**：2026-04-25
**当前版本**：v4.6.0
**项目状态**：Hub-Node 主线已闭合，Phase 0 / Phase 1 / Phase 2 通道抽象收尾完成
