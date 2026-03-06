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
