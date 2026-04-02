# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [4.4.0] - 2026-04-02

### Added

- Hub 新增 `POST /api/v1/skills/install`，支持在运行时目录在线安装 Skill，并在安装后立即刷新 registry
- Hub 新增 `POST /api/v1/skills/refresh`，用于在不重启进程的情况下重新加载运行时 Skill
- DingTalk 新增文本安装命令 `安装技能 <package> <download_url> [version]` / `install skill <package> <download_url> [version]`
- 统一配置新增 `[[channels.dingtalk.skill_installers]]` 白名单，支持按 `user_id` / `staff_id` 并可选叠加 `corp_id` 限制 DingTalk 安装入口
- `uhorse-hub` 已补齐在线安装 Skill、运行时 refresh、DingTalk 授权判定与命令解析测试覆盖

### Changed

- 在线安装当前仅接受 `source = "skillhub"` 的 Skill 包，并拒绝覆盖已存在的 Skill 目录
- DingTalk 安装入口现在会先按 `skill_installers` 校验发送者身份；未授权账号会在下载前直接被拒绝
- README / INSTALL / CHANNELS / CONFIG / SKILLS / API / RELEASE_NOTES 已统一升级到 `v4.4.0`，并补齐在线安装 Skill 与 DingTalk 权限控制说明
- GitHub release 文案将继续以 `CHANGELOG.md` 的版本段为事实源，`v4.4.0` release 无需改动现有 workflow

## [4.3.0] - 2026-04-01

### Added

- Node Desktop 新增 Settings 内的连接诊断 / 恢复能力，可直接查看 lifecycle / connection / overview、认证前提、工作区校验、最近错误与最近日志摘要，并执行最小恢复闭环
- Node Desktop 新增 `GET /api/connection/diagnostics` 与 `POST /api/connection/recover` 本地 API，供桌面 Settings 页面读取诊断状态与触发恢复动作
- Node Desktop 新增 macOS `.pkg` 与 Windows installer 打包脚本，继续复用现有 `bin + web` payload
- 新增 `desktop-installer-smoke.sh`，用于校验安装后目录下的宿主 API、静态资源与 SPA 路由回退

### Changed

- DingTalk Stream 入站现在与 Web 路径统一先走 pairing 处理，绑定码消息会优先命中运行时绑定确认，而不再误入普通任务文本链路
- Node Desktop 的 DingTalk 账号绑定闭环已完成真实 acceptance 验证：JWT 引导、pairing 确认、运行时绑定、连接诊断与已绑定状态展示已全部打通
- GitHub release / nightly workflow 现在会继续上传 Node Desktop archive，并同步上传 macOS `.pkg` 与 Windows installer 产物
- README / INSTALL / scripts / testing / release 说明已更新为当前 Node Desktop 交付边界与绑定验收路径：archive + macOS `.pkg` + Windows installer，pairing 为主路径，`notification_bindings` 仅作为兼容 seed/fallback

## [4.1.3] - 2026-03-29

### Changed

- 正式发布前的仓库入口与包元数据已统一指向当前真实仓库 `https://github.com/kts-kris/uHorse`
- 补跑并确认 `cargo test --workspace`、`./scripts/package-node-desktop.sh`、`./scripts/desktop-smoke.sh` 与 `cargo build --release -p uhorse-hub -p uhorse-node-desktop` 发布基线通过
- 基于当前 HEAD 的正式发布事实已收口为 `v4.1.3`，避免已发布 `v4.1.2` tag 与后续修正文档/元数据脱节

## [4.1.2] - 2026-03-29

### Changed

- README / INSTALL / CHANNELS / scripts / release 说明已统一升级到 `v4.1.2` 口径，并与当前 Hub-Node、DingTalk、Node Desktop 实现保持一致
- `memory / agent / skill` 的分层共享链文档已与当前实现对齐为 `global / tenant / enterprise / department / role / user / session`
- 任务上下文、运行时 session 与 Web API 文档已对齐当前 `execution_workspace_id`、`collaboration_workspace_id`、`CollaborationWorkspace` 与 `/api/v1/sessions*` 返回结构

## [4.1.1] - 2026-03-27

### Added

- DingTalk 自然语言请求现在可以规划为受控 `BrowserCommand`，支持打开公共 `http/https` 页面并通过 Hub → Node → Hub 链路回传页面文本结果
- Hub 已为浏览器目标增加本地安全校验，拒绝 `file://`、localhost、私网地址和其他越界目标
- `uhorse-node-runtime` 已接入正式浏览器执行路径，`uhorse-node-desktop` 默认启用 `browser` feature，并可通过 `CommandType::Browser` 参与能力路由
- GitHub release / nightly workflow 现在会为 `uhorse-hub` 与 `uhorse-node-desktop` 生成主流平台 archive 产物
- Node Desktop DingTalk 通知镜像当前已支持 pairing 驱动的运行时绑定闭环，`channels.dingtalk.notification_bindings` 调整为兼容 seed/fallback 说明

### Changed

- Node Desktop 当前正式交付边界已收口为 `bin + web` archive，配套 `package-node-desktop.sh`、`desktop-smoke.sh` 与 release artifacts
- README / INSTALL / CHANNELS / scripts / release 说明已统一到 `v4.1.1` 口径，并与当前 Hub-Node、DingTalk、Node Desktop 实现保持一致
- 每日构建与正式发布链路已统一使用 `Cargo.toml` 版本与 `CHANGELOG.md` 版本段作为发布事实源
- `memory / agent / skill` 的 4.1 叙事已升级为 `global / tenant / enterprise / department / role / user / session` 分层共享链，而不是旧单体 Agent 平台回归
- 任务上下文与 runtime session 已显式区分稳定 `execution_workspace_id` 和 Hub 侧逻辑 `collaboration_workspace_id` / `CollaborationWorkspace`
- runtime API 与 Web UI 已以 `source_layer`、`source_scope` 暴露来源感知信息，便于区分同名多来源资源；`/api/v1/sessions*` 也已返回 `namespace`、`memory_context_chain`、`visibility_chain` 与 `collaboration_workspace`

### Not Included

- `v4.1.1` 不包含原生 `.app/.dmg`、签名、公证、安装器或拖拽安装体验
- `v4.1.1` 不表示旧时代 `agent / skill / memory` 独立平台全面回归；当前文档只描述现有 Hub-Node 主线里的分层 runtime 能力
- `v4.1.1` 不把 legacy `uhorse` 单体路径恢复为主交付物；当前主交付物仍是 `uhorse-hub` 与 `uhorse-node-desktop`

## [4.0.0] - 2026-03-18

### Added

#### Hub-Node Distributed Architecture
- **uhorse-protocol**: Hub-Node 通信协议
  - 消息类型定义 (HubToNode, NodeToHub)
  - 二进制编解码 (MessageCodec)
  - 命令系统 (Shell, File, Edit, Write, Task, LLM, Info)
  - 任务上下文与优先级调度
  - 节点能力与状态管理

- **uhorse-node**: 本地节点
  - 任务执行引擎 (TaskExecutor)
  - 文件系统操作 (FileOps)
  - Shell 命令执行
  - 工作空间管理
  - 与 Hub 的 WebSocket 连接
  - 心跳与健康报告

- **uhorse-hub**: 云端 Hub
  - 多节点管理与调度
  - 优先级任务队列
  - 负载均衡策略
  - 节点健康监控
  - 统计信息收集

#### Security Features
- **JWT 认证**: NodeAuthenticator
  - 令牌签发与验证
  - 令牌刷新机制
  - 节点认证状态管理

- **敏感操作审批**: SensitiveOperationApprover
  - 5 类敏感操作检测 (file_delete, system_command, network_access, credential_access, config_change)
  - 审批流程 (请求、通过、拒绝)
  - 幂等性检查

- **字段加密**: HubFieldEncryptor
  - AES-GCM 加密
  - JSON 字段加密
  - 主密钥管理

- **TLS 配置**: HubTlsConfig
  - 证书路径配置
  - 安全传输支持

### Testing
- **端到端测试**: 12 个测试覆盖 Hub-Node 通信
- **集成测试**: 7 个测试覆盖 Hub 基础功能
- **安全测试**: 26 个测试覆盖 JWT、加密、审批
- **性能基准**: 8 个基准测试 (criterion)

### Performance
- 任务提交吞吐量优化
- 并发任务处理
- 优先级调度效率

## [3.0.0] - 2026-03-10

### Added

#### Enterprise Infrastructure
- **uhorse-discovery**: 服务发现 (etcd/consul)
- **uhorse-cache**: 分布式缓存 (Redis)
- **uhorse-queue**: 消息队列 (NATS)
- **uhorse-gdpr**: GDPR 合规 (数据导出、删除、同意管理)
- **uhorse-governance**: 数据治理 (分类、保留策略)
- **uhorse-backup**: 备份恢复 (自动备份、加密)
- **uhorse-sso**: SSO/OAuth2/OIDC/SAML
- **uhorse-siem**: SIEM 集成 (Splunk/Datadog)
- **uhorse-webhook**: Webhook 增强 (重试、签名)
- **uhorse-integration**: 第三方集成 (Jira/GitHub/Slack)

#### High Availability
- 服务注册与发现
- 负载均衡策略 (轮询、加权、健康感知)
- 分布式配置中心
- 自动故障转移

#### Scalability
- 数据库分片
- 读写分离
- 分布式会话缓存
- 令牌黑名单持久化
- 异步任务队列

#### Compliance
- GDPR/CCPA 合规
- 数据分类 (4 级敏感度)
- 自动备份与恢复
- 审计日志持久化

## [2.0.0] - 2026-03-05

### Added

#### Real-time Communication
- **WebSocket Support**: Full bidirectional real-time communication
  - Connection management with heartbeat and reconnection
  - Room-based pub/sub (global, agent, session)
  - Event broadcasting for messages, state changes, task progress
- **SSE (Server-Sent Events)**: Streaming event delivery
  - `/api/v1/events` endpoint for real-time updates
  - `/api/v1/chat/stream` for LLM streaming responses
  - Keep-alive support

#### Frontend Management UI
- **Dashboard**: System overview with metrics and statistics
- **Agents Page**: Agent CRUD, enable/disable, configuration
- **Skills Page**: Skill management with parameter definitions
- **Sessions Page**: Session list, details, message history
- **Channels Page**: Channel status monitoring and configuration
- **Settings Page**: System configuration with tabs
  - General settings (server, logging)
  - LLM settings (model, API key, parameters)
  - Security settings (JWT, rate limiting, CORS)

#### Enterprise Features
- **RBAC (Role-Based Access Control)**:
  - Roles: Admin, Operator, Viewer
  - Resources: Agent, Skill, Session, Channel, System, Tenant
  - Actions: Create, Read, Update, Delete, Execute, Manage
- **Audit Logging**:
  - Operation logging with user/IP tracking
  - Query API with filtering and pagination
  - Export functionality (JSON/CSV)
- **Multi-tenancy**:
  - Tenant isolation with TenantId
  - Resource quotas (agents, skills, messages, storage)
  - Tenant plans: Free, Pro, Enterprise
  - Usage tracking and billing support

#### Multi-modal Support (uhorse-multimodal crate)
- **STT (Speech-to-Text)**: OpenAI Whisper integration
  - Multi-language support
  - Automatic language detection
- **TTS (Text-to-Speech)**: OpenAI TTS integration
  - 6 voice options: alloy, echo, fable, onyx, nova, shimmer
  - Adjustable speed
- **Vision**: Image understanding
  - OpenAI GPT-4V support
  - Anthropic Claude Vision support
  - Base64 and URL image inputs
- **Document Parsing**:
  - PDF text extraction
  - Word (DOCX) parsing
  - Excel (XLSX) reading
  - Markdown/JSON/CSV support

### Changed
- Improved API handler implementations with full CRUD operations
- Enhanced channel implementations with better error handling
- Updated CI configuration with stricter clippy checks

### Dependencies
- Added `async-stream` for streaming support
- Added `futures` for async utilities
- Added `base64` for image encoding
- Added `utoipa` and `utoipa-swagger-ui` for OpenAPI docs

## [0.1.0] - 2025-03-04

### Added

#### Core Infrastructure
- **uhorse-core**: Core types, traits, and protocol definitions
- **uhorse-gateway**: HTTP/WebSocket gateway layer with session management
- **uhorse-agent**: Agent management with independent workspaces
- **uhorse-llm**: LLM abstraction layer supporting multiple providers
- **uhorse-tool**: Tool execution with MCP protocol support
- **uhorse-storage**: Storage layer with SQLite and JSONL backends
- **uhorse-security**: Security layer with JWT authentication and device pairing
- **uhorse-scheduler**: Cron-based task scheduling
- **uhorse-observability**: Observability with tracing, metrics, and audit logs
- **uhorse-config**: Configuration management with interactive wizard

#### Channel Support
- **Telegram**: Full Bot API support with webhook integration
- **钉钉 (DingTalk)**: Enterprise messaging with rich text support
- **飞书 (Feishu/Lark)**: Rich text and interactive card messages
- **企业微信 (WeCom)**: Enterprise internal communication
- **Slack**: Slash commands and interactive components
- **Discord**: Bot integration with embed messages
- **WhatsApp**: WhatsApp Business API integration

#### Features
- Multi-channel unified gateway
- SKILL.md driven skill system
- Structured memory system (SOUL.md, MEMORY.md, USER.md)
- Enterprise-grade security (JWT, device pairing, approval workflow)
- Interactive configuration wizard
- Docker and docker-compose support
- GitHub Actions CI/CD pipeline
- Cross-platform binary releases (Linux, macOS, Windows)

### Performance

- 100K+ concurrent connections
- <1ms P99 latency
- ~30ms cold start time
- 5-20MB memory footprint
- ~15MB binary size (release build)

### Documentation

- README.md with quick start guide
- INSTALL.md detailed installation guide
- API.md REST API reference
- CHANNELS.md channel configuration guide
- CONFIG.md configuration reference
- CONTRIBUTING.md contribution guidelines
- SECURITY.md security policy
- COMPARISON_OPENCLAW.md comparison with OpenClaw

[Unreleased]: https://github.com/kts-kris/uHorse/compare/v4.4.0...HEAD
[4.4.0]: https://github.com/kts-kris/uHorse/compare/v4.3.0...v4.4.0
[4.3.0]: https://github.com/kts-kris/uHorse/compare/v4.1.3...v4.3.0
[4.1.3]: https://github.com/kts-kris/uHorse/compare/v4.1.2...v4.1.3
[4.1.2]: https://github.com/kts-kris/uHorse/compare/v4.1.1...v4.1.2
[4.1.1]: https://github.com/kts-kris/uHorse/compare/v4.0.0...v4.1.1
[4.0.0]: https://github.com/kts-kris/uHorse/compare/v3.0.0...v4.0.0
[3.0.0]: https://github.com/kts-kris/uHorse/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/kts-kris/uHorse/compare/v0.1.0...v2.0.0
[0.1.0]: https://github.com/kts-kris/uHorse/releases/tag/v0.1.0
