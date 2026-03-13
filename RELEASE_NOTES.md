## uHorse 3.0.0 发布

**发布日期**: 2025-03-13

### 重大更新

uHorse 3.0.0 是一个里程碑版本，完成了从"企业级多渠道 AI 网关"到"企业级 AI 基础设施平台"的全面升级。

#### 核心指标提升

| 维度 | 2.0 基准 | 3.0 达成 | 提升 |
|------|----------|----------|------|
| **高可用性** | 40% | 95% | +55% |
| **可扩展性** | 40% | 95% | +55% |
| **安全合规** | 50% | 100% | +50% |
| **数据治理** | 40% | 100% | +60% |
| **API 标准** | 60% | 100% | +40% |
| **企业集成** | 30% | 100% | +70% |

---

### 新增功能 (6 个 Phase)

#### Phase 1: 高可用性基础设施
- ✅ **服务发现**: etcd + Consul 双后端
- ✅ **负载均衡**: 4 种策略 (轮询/加权/健康感知/最少连接)
- ✅ **分布式配置**: 配置中心 + 热加载 + 版本管理
- ✅ **故障转移**: 自动/手动/优先级策略

#### Phase 2: 可扩展性架构
- ✅ **数据库分片**: 按 tenant_id 分片 + 读写分离
- ✅ **分布式缓存**: Redis 集成 (会话/令牌黑名单)
- ✅ **消息队列**: NATS 集成 (任务队列/死信队列)
- ✅ **缓存策略**: LRU/LFU/TTL

#### Phase 3: 安全合规体系
- ✅ **传输加密**: TLS 1.3 + Let's Encrypt 自动证书
- ✅ **存储加密**: SQLCipher + 字段级加密
- ✅ **GDPR 合规**: 数据导出/删除/同意管理
- ✅ **审计增强**: 持久化 + 防篡改签名

#### Phase 4: 数据治理体系
- ✅ **数据分类**: 4 级敏感度 (Public/Internal/Confidential/Restricted)
- ✅ **保留策略**: 自动过期删除
- ✅ **备份恢复**: 增量/完整备份 + AES-256-GCM 加密
- ✅ **灾难恢复**: PITR + 跨区域复制

#### Phase 5: API 标准体系
- ✅ **OpenAPI 3.0**: utoipa 集成 + Swagger UI + ReDoc
- ✅ **客户端生成**: TypeScript/Go/Python/Rust
- ✅ **API 版本管理**: URL 版本 + 废弃通知
- ✅ **Rate Limiting**: 全局/用户/端点/分布式

#### Phase 6: 企业集成体系
- ✅ **SSO/OAuth2**: 授权服务器 + OIDC + SAML 2.0
- ✅ **多 IdP 集成**: Okta/Auth0/Azure AD/Google Workspace
- ✅ **SIEM 集成**: Splunk HEC + Datadog Logs API
- ✅ **第三方集成**: Jira/GitHub/Slack

---

### 新增模块 (11 个 crate)

| Crate | 功能 |
|-------|------|
| `uhorse-discovery` | 服务发现 (etcd/Consul) |
| `uhorse-cache` | 分布式缓存 (Redis) |
| `uhorse-queue` | 消息队列 (NATS) |
| `uhorse-gdpr` | GDPR 合规 |
| `uhorse-governance` | 数据治理 |
| `uhorse-backup` | 备份恢复 |
| `uhorse-sso` | SSO/OAuth2/OIDC/SAML |
| `uhorse-siem` | SIEM 集成 |
| `uhorse-webhook` | Webhook 增强 |
| `uhorse-integration` | 第三方集成 |
| `uhorse-observability` | 可观测性增强 |

---

### 测试覆盖

- **总测试数**: 329+
- **状态**: ✅ 全部通过

---

### 升级指南

```bash
# 拉取最新代码
git pull
git checkout v3.0.0

# 构建项目
cargo build --release

# 启动完整栈
docker-compose up -d

# 健康检查
curl http://localhost:8080/health/live
```

---

## uHorse 2.0.0 发布

### 重大更新

uHorse 2.0.0 是一个重要里程碑版本，带来了企业级多渠道 AI 网关的完整实现。

---

## 新功能

### 实时通信
- **WebSocket 支持**: 全双工实时通信
  - 连接管理（心跳、重连、会话绑定）
  - 房间机制（全局/Agent/Session 分组）
  - 事件推送（消息、状态变更、任务进度）
- **SSE 流式响应**: 服务器推送事件
  - /api/v1/events 事件流端点
  - /api/v1/chat/stream LLM 流式聊天
  - Keep-alive 支持

### 前端管理界面
- **Dashboard**: 系统概览、统计数据
- **Agents 页面**: Agent 管理CRUD
- **Skills 页面**: 技能管理
- **Sessions 页面**: 会话列表、消息历史
- **Channels 页面**: 通道状态监控
- **Settings 页面**: 系统配置
  - 通用设置（服务器、日志）
  - LLM 设置（模型、API Key、参数）
  - 安全设置（JWT、限流、CORS）

### 企业级特性
- **RBAC 权限控制**:
  - 角色：Admin / Operator / Viewer
  - 资源：Agent / Skill / Session / Channel / System / Tenant
  - 操作：Create / Read / Update / Delete / Execute / Manage
- **审计日志**:
  - 操作日志记录（用户/IP 追踪）
  - 查询 API（过滤、分页）
  - 导出功能（JSON/CSV）
- **多租户架构**:
  - 租户隔离（TenantId）
  - 资源配额（Agent/技能/消息/存储）
  - 租户计划：Free / Pro / Enterprise

### 多模态支持
- **STT 语音转文字**: OpenAI Whisper 集成
- **TTS 文字转语音**: OpenAI TTS 集成（6 种音色）
- **Vision 图像理解**: GPT-4V / Claude Vision 支持
- **文档解析**: PDF / DOCX / XLSX / MD / JSON / CSV

---

## 技术改进

### 新增依赖
- async-stream - 流式处理
- futures - 异步工具
- base64 - 图像编码
- utoipa + utoipa-swagger-ui - OpenAPI 文档

### 新增模块
- crates/uhorse-multimodal/ - 多模态支持 crate
- crates/uhorse-gateway/src/auth/ - 认证模块（RBAC/审计/多租户）
- crates/uhorse-gateway/src/websocket.rs - WebSocket 管理
- web/src/pages/ - 前端管理页面

---

## 升级指南

```bash
# 拉取最新代码
git pull
git checkout v2.0.0

# 构建项目
cargo build --release

# 启动服务
cargo run --release

# 启动前端
cd web && npm install && npm run build
```

---

## 致谢

感谢所有贡献者的辛勤工作！
