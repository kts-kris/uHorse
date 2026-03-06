# uHorse 2.0 版本规划

## 版本愿景

**uHorse 2.0** 定位为"企业级多渠道 AI 网关"，实现：
- 🎯 **生产就绪**: 完整的业务逻辑实现，非框架代码
- 🔌 **全通道覆盖**: 7 种主流消息通道完整实现
- 🎙️ **多模态支持**: 文本 + 语音(STT/TTS) + 图像(Vision) + 文件解析
- 👥 **多租户架构**: 租户隔离、资源配额、独立配置
- 📊 **可观测性**: 完整的监控、告警、追踪体系
- 🛡️ **企业级安全**: RBAC、审计、合规支持

---

## 实施进度

| Phase | 名称 | 状态 | 完成日期 |
|-------|------|------|----------|
| Phase 1 | API Handler 实现 | ✅ 已完成 | 2026-03-05 |
| Phase 2 | 核心通道实现 | ✅ 已完成 | 2026-03-05 |
| Phase 3 | 实时通信 | ✅ 已完成 | 2026-03-05 |
| Phase 4 | 前端管理页面 | ✅ 已完成 | 2026-03-05 |
| Phase 5 | 企业级特性 | ✅ 已完成 | 2026-03-05 |
| Phase 6 | 多模态支持 | ✅ 已完成 | 2026-03-05 |

---

## 功能清单

### Phase 1: API 完善 (P0) ✅

#### 1.1 API Handler 实现
| 模块 | 文件 | 功能 | 状态 |
|------|------|------|------|
| agents | `uhorse-gateway/src/api/handlers/agents.rs` | Agent CRUD、启停、配置 | ✅ |
| skills | `uhorse-gateway/src/api/handlers/skills.rs` | 技能注册、执行、权限 | ✅ |
| sessions | `uhorse-gateway/src/api/handlers/sessions.rs` | 会话列表、详情、消息历史 | ✅ |
| files | `uhorse-gateway/src/api/handlers/files.rs` | Agent 文件系统管理 | ✅ |
| channels | `uhorse-gateway/src/api/handlers/channels.rs` | 通道状态、配置、测试 | ✅ |
| marketplace | `uhorse-gateway/src/api/handlers/marketplace.rs` | 技能市场搜索、安装 | ✅ |

#### 1.2 OpenAPI 文档
- ✅ 集成 `utoipa` 生成 OpenAPI 3.0 规范
- ✅ 添加 Swagger UI (`/docs`)
- ✅ 生成 TypeScript 客户端

### Phase 2: 通道实现 (P0) ✅

| 通道 | 优先级 | 功能 | 状态 |
|------|--------|------|------|
| **Telegram** | P0 | Bot API、Webhook、命令处理 | ✅ |
| **钉钉** | P0 | 企业内部应用、Stream 模式 | ✅ |
| **飞书** | P0 | 自建应用、事件订阅 | ✅ |
| **企业微信** | P1 | 应用管理、消息回调 | ✅ |
| **Slack** | P1 | App、Slash Commands、Events | ✅ |
| **Discord** | P2 | Bot、Gateway、Interactions | ✅ |
| **WhatsApp** | P2 | Business API、Webhook | ✅ |

### Phase 3: 实时通信 (P0) ✅

#### 3.1 WebSocket 完善
- ✅ 连接管理 (心跳、重连、会话绑定)
- ✅ 事件推送 (消息、状态变更、任务进度)
- ✅ 房间机制 (按 Agent/Session 分组)

#### 3.2 流式响应
- ✅ SSE 端点 (`/api/v1/events`)
- ✅ LLM 流式输出支持 (`/api/v1/chat/stream`)
- ✅ 前端流式渲染组件

### Phase 4: 前端完善 (P1) ✅

| 页面 | 功能 | 状态 |
|------|------|------|
| Agent 管理 | 列表、创建、配置、对话测试 | ✅ |
| 技能管理 | 技能列表、安装、配置、测试 | ✅ |
| Session 管理 | 会话列表、详情、消息历史 | ✅ |
| 通道管理 | 通道配置、状态监控、测试 | ✅ |
| 系统设置 | 系统配置、用户管理、日志查看 | ✅ |

### Phase 5: 企业级特性 (P1) ✅

#### 5.1 权限系统 (RBAC)
- ✅ 角色定义 (admin/operator/viewer)
- ✅ 资源权限 (Agent/Skill/Channel/Session/System/Tenant)
- ✅ API 鉴权中间件

#### 5.2 审计日志
- ✅ 操作日志记录
- ✅ 日志查询 API
- ✅ 日志导出

#### 5.3 多租户架构
- ✅ **租户隔离**: TenantId 贯穿所有资源
- ✅ **资源配额**: Agent 数量、消息量、存储空间限制
- ✅ **独立配置**: 每个租户独立的 LLM 配置、通道配置
- ✅ **计费支持**: 使用量统计、账单生成

### Phase 6: 多模态支持 (P1) ✅

#### 6.1 语音处理
- ✅ **STT (语音转文字)**: OpenAI Whisper API 集成
- ✅ **TTS (文字转语音)**: OpenAI TTS API 集成 (6 种音色)

#### 6.2 图像理解
- ✅ **Vision API**: OpenAI GPT-4V、Anthropic Claude Vision
- ✅ **图像处理**: Base64 编码、URL 支持

#### 6.3 文件解析
- ✅ **PDF**: 文本提取
- ✅ **Word (DOCX)**: 内容解析
- ✅ **Excel (XLSX)**: 表格读取
- ✅ **Markdown/JSON/CSV**: 格式解析

---

## 已实现的技术改进

### 新增依赖
```toml
utoipa = "4.0"
utoipa-swagger-ui = "4.0"
axum-extra = { version = "0.9", features = ["typed-header"] }
async-stream = "0.3"
futures = "0.3"
base64 = "0.21"
bytes = "1.0"
```

### 新增模块
```
crates/uhorse-multimodal/
├── Cargo.toml
└── src/
    ├── lib.rs        # 模块导出
    ├── stt.rs        # 语音转文字 (Whisper)
    ├── tts.rs        # 文字转语音
    ├── vision.rs     # 图像理解
    └── document.rs   # 文档解析

crates/uhorse-gateway/src/
├── websocket.rs      # WebSocket 连接管理
├── auth/
│   ├── mod.rs       # 认证模块
│   ├── rbac.rs      # 权限控制
│   ├── audit.rs     # 审计日志
│   └── tenant.rs    # 多租户

web/src/pages/
├── Agents.tsx       # Agent 管理
├── Skills.tsx       # 技能管理
├── Sessions.tsx     # 会话管理
├── Channels.tsx     # 通道管理
└── Settings.tsx     # 系统设置
```

---

## API 端点

### 实时通信
- `GET /ws` - WebSocket 连接
- `GET /api/v1/events` - SSE 事件流
- `POST /api/v1/chat/stream` - LLM 流式聊天
- `GET /api/v1/connections` - 活跃连接数

### 企业级功能
- `GET /api/v1/rbac/permissions` - 获取权限列表
- `POST /api/v1/rbac/check` - 检查权限
- `GET /api/v1/audit/logs` - 查询审计日志
- `GET /api/v1/audit/export` - 导出审计日志
- `GET /api/v1/tenants` - 租户列表
- `POST /api/v1/tenants` - 创建租户
- `GET /api/v1/tenants/{id}/quota` - 查询配额使用

### 多模态
- `POST /api/v1/stt` - 语音转文字
- `POST /api/v1/tts` - 文字转语音
- `POST /api/v1/vision` - 图像理解
- `POST /api/v1/document/parse` - 文档解析

---

## 验证方案

```bash
# 运行所有测试
cargo test --all

# 启动服务
cargo run

# WebSocket 测试
wscat -c ws://localhost:8080/ws

# SSE 测试
curl -N http://localhost:8080/api/v1/events

# 前端开发
cd web && npm run dev
```

---

## 风险评估

| 风险 | 影响 | 缓解措施 | 状态 |
|------|------|----------|------|
| 通道 API 变更 | 高 | 抽象层隔离，版本锁定 | ✅ 已处理 |
| LLM 流式兼容性 | 中 | 统一适配器模式 | ✅ 已处理 |
| 前端复杂度 | 中 | 使用 Ant Design 组件库 | ✅ 已处理 |
| 测试覆盖不足 | 高 | CI 强制检查 | ✅ 已配置 |
