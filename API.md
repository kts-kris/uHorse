# uHorse API 使用指南

## 目录

- [API 概述](#api-概述)
- [认证方式](#认证方式)
- [健康检查](#健康检查)
- [WebSocket API](#websocket-api)
- [HTTP REST API](#http-rest-api)
- [工具调用](#工具调用)
- [错误处理](#错误处理)
- [使用示例](#使用示例)

---

## API 概述

uHorse 提供两种 API 接口：

1. **WebSocket API** - 实时双向通信
2. **HTTP REST API** - 标准 HTTP 请求

### 基础信息

- **Base URL**: `http://localhost:8080`
- **API 版本**: v1
- **数据格式**: JSON
- **字符编码**: UTF-8

---

## 认证方式

### JWT Token 认证

```bash
# 1. 获取访问令牌
curl -X POST http://localhost:8080/api/v1/auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "device_id": "device_123",
    "pairing_code": "123456"
  }'

# 响应
{
  "access_token": "eyJhbGciOiJIUzI1NiIs...",
  "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_in": 86400,
  "token_type": "Bearer"
}

# 2. 使用令牌访问 API
curl http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."
```

### API Key 认证（可选）

```bash
curl http://localhost:8080/api/v1/sessions \
  -H "X-API-Key: your_api_key_here"
```

---

## 健康检查

### 存活性检查

```bash
curl http://localhost:8080/health/live
```

**响应:**
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### 就绪性检查

```bash
curl http://localhost:8080/health/ready
```

**响应:**
```json
{
  "status": "ready",
  "version": "0.1.0",
  "checks": {
    "database": "ok",
    "redis": "ok"
  }
}
```

---

## WebSocket API

### 连接 WebSocket

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');

// 监听连接打开
ws.onopen = () => {
  console.log('WebSocket connected');

  // 发送握手
  ws.send(JSON.stringify({
    type: 'handshake',
    data: {
      version: '1.0',
      capabilities: ['tools', 'channels']
    }
  }));
};

// 监听消息
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log('Received:', message);

  // 处理不同类型的消息
  switch(message.type) {
    case 'handshake_response':
      console.log('Handshake successful');
      break;
    case 'event':
      handleEvent(message.data);
      break;
    case 'response':
      handleResponse(message.data);
      break;
    case 'error':
      console.error('Error:', message.data);
      break;
  }
};

// 监听错误
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

// 监听关闭
ws.onclose = () => {
  console.log('WebSocket disconnected');
};
```

### 握手协议

**请求:**
```json
{
  "type": "handshake",
  "data": {
    "version": "1.0",
    "capabilities": ["tools", "channels"],
    "client_info": {
      "name": "uHorse Client",
      "version": "1.0.0"
    }
  }
}
```

**响应:**
```json
{
  "type": "handshake_response",
  "data": {
    "server_info": {
      "name": "uHorse",
      "version": "0.1.0"
    },
    "features": {
      "tools": true,
      "channels": true,
      "sessions": true
    },
    "heartbeat_interval": 30
  }
}
```

### 发送消息

```javascript
// 发送文本消息
ws.send(JSON.stringify({
  type: 'message',
  data: {
    session_id: 'session_123',
    content: {
      type: 'text',
      text': 'Hello, uHorse!'
    },
    role: 'user'
  }
}));
```

### 工具调用

```javascript
// 调用计算器工具
ws.send(JSON.stringify({
  type: 'tool_call',
  data: {
    session_id: 'session_123',
    tool_id: 'calculator',
    parameters: {
      expression: '2 + 2'
    }
  }
}));
```

---

## HTTP REST API

### 1. 会话管理

#### 创建会话

```bash
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "isolation": "moderate",
    "metadata": {
      "user_id": "user_123"
    }
  }'
```

**响应:**
```json
{
  "session_id": "session_abc123",
  "status": "active",
  "created_at": "2026-03-02T12:00:00Z",
  "isolation": "moderate"
}
```

#### 获取会话列表

```bash
curl http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**响应:**
```json
{
  "sessions": [
    {
      "session_id": "session_abc123",
      "status": "active",
      "created_at": "2026-03-02T12:00:00Z",
      "message_count": 15
    }
  ],
  "total": 1,
  "page": 1,
  "page_size": 20
}
```

#### 获取会话详情

```bash
curl http://localhost:8080/api/v1/sessions/session_abc123 \
  -H "Authorization: Bearer YOUR_TOKEN"
```

#### 删除会话

```bash
curl -X DELETE http://localhost:8080/api/v1/sessions/session_abc123 \
  -H "Authorization: Bearer YOUR_TOKEN"
```

### 2. 消息管理

#### 发送消息

```bash
curl -X POST http://localhost:8080/api/v1/sessions/session_abc123/messages \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "content": {
      "type": "text",
      "text": "帮我计算 2 + 2"
    },
    "role": "user"
  }'
```

**响应:**
```json
{
  "message_id": "msg_456",
  "session_id": "session_abc123",
  "role": "assistant",
  "content": {
    "type": "text",
    "text": "2 + 2 = 4"
  },
  "created_at": "2026-03-02T12:05:00Z"
}
```

#### 获取消息历史

```bash
curl http://localhost:8080/api/v1/sessions/session_abc123/messages \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -G -d "limit=50" \
  -d "before=msg_456"
```

### 3. 工具调用

#### 执行工具

```bash
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "calculator",
    "parameters": {
      "expression": "10 * 5"
    }
  }'
```

**响应:**
```json
{
  "tool": "calculator",
  "call_id": "call_789",
  "status": "success",
  "result": 50,
  "duration_ms": 5
}
```

#### 获取可用工具列表

```bash
curl http://localhost:8080/api/v1/tools \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**响应:**
```json
{
  "tools": [
    {
      "id": "calculator",
      "name": "计算器",
      "description": "执行数学计算",
      "parameters": {
        "expression": {
          "type": "string",
          "description": "数学表达式",
          "required": true
        }
      }
    },
    {
      "id": "http",
      "name": "HTTP 请求",
      "description": "发送 HTTP 请求"
    }
  ]
}
```

### 4. 通道管理

#### 发送消息到通道

```bash
curl -X POST http://localhost:8080/api/v1/channels/telegram/send \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": "123456789",
    "text": "Hello from uHorse!",
    "parse_mode": "Markdown"
  }'
```

#### 获取通道状态

```bash
curl http://localhost:8080/api/v1/channels/status \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**响应:**
```json
{
  "channels": {
    "telegram": {
      "connected": true,
      "webhook": "active"
    },
    "slack": {
      "connected": false,
      "error": "Bot token not configured"
    }
  }
}
```

---

## 工具调用

### 内置工具

#### 1. 计算器

```bash
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "calculator",
    "parameters": {
      "expression": "2 * (3 + 4)"
    }
  }'
```

**结果:** `14`

#### 2. HTTP 请求

```bash
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "http",
    "parameters": {
      "url": "https://api.github.com",
      "method": "GET"
    }
  }'
```

#### 3. 搜索

```bash
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "search",
    "parameters": {
      "query": "Rust programming"
    }
  }'
```

#### 4. 日期时间

```bash
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "datetime",
    "parameters": {
      "action": "current"
    }
  }'
```

---

## 错误处理

### 错误响应格式

```json
{
  "error": {
    "code": "INVALID_TOKEN",
    "message": "Invalid or expired token",
    "details": {
      "hint": "Try refreshing your access token"
    }
  }
}
```

### 错误码

| 错误码 | 说明 |
|--------|------|
| `INVALID_TOKEN` | 令牌无效或过期 |
| `INSUFFICIENT_PERMISSIONS` | 权限不足 |
| `SESSION_NOT_FOUND` | 会话不存在 |
| `TOOL_NOT_FOUND` | 工具不存在 |
| `INVALID_PARAMETERS` | 参数无效 |
| `RATE_LIMIT_EXCEEDED` | 超出速率限制 |

---

## 使用示例

### Python 示例

```python
import requests
import websocket
import json

BASE_URL = "http://localhost:8080"
TOKEN = "your_access_token"

def create_session():
    """创建会话"""
    response = requests.post(
        f"{BASE_URL}/api/v1/sessions",
        headers={"Authorization": f"Bearer {TOKEN}"},
        json={"isolation": "moderate"}
    )
    return response.json()["session_id"]

def send_message(session_id, text):
    """发送消息"""
    response = requests.post(
        f"{BASE_URL}/api/v1/sessions/{session_id}/messages",
        headers={"Authorization": f"Bearer {TOKEN}"},
        json={
            "content": {"type": "text", "text": text},
            "role": "user"
        }
    )
    return response.json()

# 使用示例
session_id = create_session()
result = send_message(session_id, "你好")
print(result["content"]["text"])
```

### JavaScript 示例

```javascript
const BASE_URL = 'http://localhost:8080';
const TOKEN = 'your_access_token';

async function createSession() {
  const response = await fetch(`${BASE_URL}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${TOKEN}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({ isolation: 'moderate' })
  });
  return await response.json();
}

async function sendMessage(sessionId, text) {
  const response = await fetch(
    `${BASE_URL}/api/v1/sessions/${sessionId}/messages`,
    {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${TOKEN}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        content: { type: 'text', text },
        role: 'user'
      })
    }
  );
  return await response.json();
}

// 使用示例
(async () => {
  const session = await createSession();
  const result = await sendMessage(session.session_id, 'Hello');
  console.log(result.content.text);
})();
```

### cURL 示例

```bash
# 设置 token
export TOKEN="your_access_token"

# 创建会话
SESSION=$(curl -s -X POST http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"isolation":"moderate"}' | jq -r '.session_id')

# 发送消息
curl -X POST http://localhost:8080/api/v1/sessions/$SESSION/messages \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "content": {"type": "text", "text": "Hello"},
    "role": "user"
  }' | jq '.'
```

---

## 下一步

- [配置指南](CONFIG.md)
- [通道集成指南](CHANNELS.md)
- [部署指南](deployments/DEPLOYMENT.md)
