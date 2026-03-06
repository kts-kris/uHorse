# uHorse 2.0 新功能指南

本文档详细介绍 uHorse 2.0 版本新增的核心功能。

---

## 目录

1. [实时通信](#1-实时通信)
2. [前端管理界面](#2-前端管理界面)
3. [企业级特性](#3-企业级特性)
4. [多模态支持](#4-多模态支持)

---

## 1. 实时通信

### 1.1 WebSocket 支持

uHorse 2.0 提供完整的 WebSocket 支持，实现实时双向通信。

#### 连接端点

```
ws://localhost:8080/ws?client_id=xxx&session_id=xxx&agent_id=xxx
```

#### 查询参数

| 参数 | 说明 |
|------|------|
| `client_id` | 客户端唯一标识（可选） |
| `session_id` | 绑定的会话 ID（可选） |
| `agent_id` | 绑定的 Agent ID（可选） |

#### 消息格式

**客户端发送命令：**

```json
// 心跳
{"type": "ping", "timestamp": 1234567890}

// 订阅房间
{"type": "subscribe", "room": "agent:xxx"}

// 取消订阅
{"type": "unsubscribe", "room": "session:xxx"}

// 发送消息
{"type": "send", "session_id": "xxx", "content": "Hello"}
```

**服务端推送事件：**

```json
// 消息事件
{
  "type": "message",
  "session_id": "xxx",
  "role": "user",
  "content": "Hello",
  "timestamp": 1234567890
}

// 状态变更
{
  "type": "state_change",
  "entity_type": "agent",
  "entity_id": "xxx",
  "old_state": "idle",
  "new_state": "processing"
}

// 任务进度
{
  "type": "task_progress",
  "task_id": "xxx",
  "progress": 0.5,
  "message": "Processing..."
}

// 心跳响应
{"type": "pong", "timestamp": 1234567890}

// 错误
{"type": "error", "code": "xxx", "message": "Error description"}
```

#### 房间机制

| 房间 | 格式 | 说明 |
|------|------|------|
| 全局 | `global` | 所有系统事件 |
| Agent | `agent:{id}` | 特定 Agent 的事件 |
| Session | `session:{id}` | 特定会话的事件 |

### 1.2 SSE 流式响应

#### 事件流端点

```
GET /api/v1/events?lastEventId=xxx&rooms=agent:xxx,session:xxx
```

#### 流式聊天

```
POST /api/v1/chat/stream
Content-Type: application/json

{
  "session_id": "xxx",
  "message": "Hello",
  "model": "gpt-4"
}
```

**响应格式（SSE）：**

```
event: chunk
data: {"session_id":"xxx","content":"Hello","done":false}

event: chunk
data: {"session_id":"xxx","content":"!","done":false}

event: done
data: {"status":"completed"}
```

---

## 2. 前端管理界面

uHorse 2.0 提供基于 React + Ant Design 的现代化管理界面。

### 2.1 页面结构

| 路由 | 页面 | 功能 |
|------|------|------|
| `/dashboard` | Dashboard | 系统概览、统计数据 |
| `/agents` | Agents | Agent 管理 |
| `/skills` | Skills | 技能管理 |
| `/sessions` | Sessions | 会话管理 |
| `/channels` | Channels | 通道管理 |
| `/settings` | Settings | 系统设置 |

### 2.2 Agent 管理

- 列表展示所有 Agent
- 创建/编辑/删除 Agent
- 启用/禁用 Agent
- 配置 Agent 参数

### 2.3 技能管理

- 技能列表和详情
- 创建/编辑技能
- 技能参数定义
- 启用/禁用技能

### 2.4 会话管理

- 会话列表和筛选
- 查看消息历史
- 会话状态管理
- 按状态/Agent 筛选

### 2.5 通道管理

- 通道状态监控
- 配置管理
- 连接测试

### 2.6 系统设置

**系统概览 Tab：**
- 运行时间
- 今日消息数
- 平均响应时间
- 系统信息

**通用设置 Tab：**
- 服务地址/端口
- 日志级别
- 最大连接数

**LLM 设置 Tab：**
- 默认模型
- API Key
- 温度/Max Tokens
- 超时设置

**安全设置 Tab：**
- JWT 认证
- Token 过期时间
- 速率限制
- CORS 配置

---

## 3. 企业级特性

### 3.1 RBAC 权限控制

#### 角色定义

| 角色 | 权限 |
|------|------|
| `Admin` | 完全访问权限，包括系统配置和租户管理 |
| `Operator` | 创建/修改资源，执行操作 |
| `Viewer` | 只读权限 |

#### 资源类型

- `Agent` - Agent 资源
- `Skill` - 技能资源
- `Session` - 会话资源
- `Channel` - 通道资源
- `System` - 系统配置
- `Tenant` - 租户管理

#### 操作类型

- `Create` - 创建
- `Read` - 读取
- `Update` - 更新
- `Delete` - 删除
- `Execute` - 执行
- `Manage` - 管理

#### API 端点

```bash
# 获取所有权限
GET /api/v1/rbac/permissions

# 检查权限
POST /api/v1/rbac/check
{
  "user_id": "xxx",
  "role": "operator",
  "resource_type": "Agent",
  "action": "Create"
}
```

### 3.2 审计日志

#### 日志字段

| 字段 | 说明 |
|------|------|
| `id` | 日志 ID |
| `tenant_id` | 租户 ID |
| `user_id` | 用户 ID |
| `action` | 操作类型 |
| `resource_type` | 资源类型 |
| `resource_id` | 资源 ID |
| `details` | 详细信息 |
| `ip_address` | IP 地址 |
| `user_agent` | User Agent |
| `created_at` | 创建时间 |

#### 查询参数

```bash
GET /api/v1/audit/logs?user_id=xxx&action=Create&start=xxx&end=xxx&page=1&page_size=20
```

#### 导出日志

```bash
GET /api/v1/audit/export?format=json&start=xxx&end=xxx
```

### 3.3 多租户架构

#### 租户计划

| 计划 | Agent 数量 | 技能数量 | 每日消息 | 存储 |
|------|-----------|---------|---------|------|
| `Free` | 5 | 10 | 1,000 | 100MB |
| `Pro` | 50 | 100 | 50,000 | 10GB |
| `Enterprise` | 无限 | 无限 | 无限 | 无限 |

#### API 端点

```bash
# 创建租户
POST /api/v1/tenants
{
  "name": "Company A",
  "plan": "pro"
}

# 查询配额使用
GET /api/v1/tenants/{id}/quota

# 更新租户
PUT /api/v1/tenants/{id}

# 删除租户
DELETE /api/v1/tenants/{id}
```

---

## 4. 多模态支持

### 4.1 语音转文字 (STT)

使用 OpenAI Whisper API。

```bash
POST /api/v1/stt
Content-Type: multipart/form-data

file: <audio_file>
language: zh
model: whisper-1
```

**响应：**

```json
{
  "text": "你好，世界",
  "language": "zh",
  "duration": 5.2
}
```

### 4.2 文字转语音 (TTS)

使用 OpenAI TTS API。

```bash
POST /api/v1/tts
Content-Type: application/json

{
  "text": "你好，世界",
  "voice": "alloy",
  "model": "tts-1",
  "speed": 1.0
}
```

**可用音色：**

| 音色 | 特点 |
|------|------|
| `alloy` | 中性 |
| `echo` | 男性 |
| `fable` | 英国口音 |
| `onyx` | 深沉男性 |
| `nova` | 女性 |
| `shimmer` | 温暖女性 |

### 4.3 图像理解 (Vision)

支持 OpenAI GPT-4V 和 Anthropic Claude Vision。

```bash
POST /api/v1/vision
Content-Type: application/json

{
  "image": "data:image/png;base64,...",
  "prompt": "描述这张图片",
  "model": "gpt-4-vision-preview"
}
```

**响应：**

```json
{
  "description": "这是一张...",
  "model": "gpt-4-vision-preview"
}
```

### 4.4 文档解析

支持多种文档格式。

```bash
POST /api/v1/document/parse
Content-Type: multipart/form-data

file: <document_file>
```

**支持格式：**

| 格式 | 扩展名 |
|------|--------|
| PDF | `.pdf` |
| Word | `.docx` |
| Excel | `.xlsx` |
| Markdown | `.md` |
| JSON | `.json` |
| CSV | `.csv` |

**响应：**

```json
{
  "content": "文档内容...",
  "format": "pdf",
  "pages": 10,
  "metadata": {
    "title": "Document Title",
    "author": "Author"
  }
}
```

---

## 快速开始

### 启动服务

```bash
# 构建项目
cargo build --release

# 运行服务
cargo run

# 或使用配置文件
cargo run -- --config config.yaml
```

### 启动前端

```bash
cd web
npm install
npm run dev
```

### 访问界面

- 前端界面: http://localhost:5173
- API 文档: http://localhost:8080/docs
- WebSocket: ws://localhost:8080/ws

---

## 配置示例

### 环境变量

```bash
# 服务配置
UHORSE_HOST=0.0.0.0
UHORSE_PORT=8080
UHORSE_LOG_LEVEL=info

# LLM 配置
OPENAI_API_KEY=sk-xxx
OPENAI_API_BASE=https://api.openai.com/v1

# 安全配置
JWT_SECRET=your-secret-key
JWT_EXPIRY=3600
```

### 配置文件

```yaml
server:
  host: 0.0.0.0
  port: 8080

llm:
  default_model: gpt-4
  api_key: ${OPENAI_API_KEY}
  temperature: 0.7
  max_tokens: 4096

security:
  jwt_enabled: true
  jwt_secret: ${JWT_SECRET}
  token_expiry: 3600
  rate_limit_enabled: true
  rate_limit_per_minute: 60
```

---

## 常见问题

### Q: WebSocket 连接断开怎么办？

A: 实现自动重连机制，使用指数退避策略。

### Q: 如何添加新的通道？

A: 实现 `Channel` trait，在 `uhorse-channel` crate 中添加新模块。

### Q: 如何扩展权限系统？

A: 在 `rbac.rs` 中添加新的资源和操作类型。

---

## 更新日志

### v2.0.0 (2026-03-05)

- ✨ 新增 WebSocket 实时通信
- ✨ 新增 SSE 流式响应
- ✨ 新增前端管理界面
- ✨ 新增 RBAC 权限控制
- ✨ 新增审计日志
- ✨ 新增多租户支持
- ✨ 新增语音转文字 (STT)
- ✨ 新增文字转语音 (TTS)
- ✨ 新增图像理解 (Vision)
- ✨ 新增文档解析
