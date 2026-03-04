# uHorse API Reference

## Table of Contents

- [API Overview](#api-overview)
- [Authentication](#authentication)
- [Health Check](#health-check)
- [WebSocket API](#websocket-api)
- [HTTP REST API](#http-rest-api)
- [Tool Calling](#tool-calling)
- [Error Handling](#error-handling)
- [Usage Examples](#usage-examples)

---

## API Overview

uHorse provides two API interfaces:

1. **WebSocket API** - Real-time bidirectional communication
2. **HTTP REST API** - Standard HTTP requests

### Basic Information

| Property | Value |
|----------|-------|
| **Base URL** | `http://localhost:8080` |
| **API Version** | v1 |
| **Data Format** | JSON |
| **Character Encoding** | UTF-8 |

---

## Authentication

### JWT Token

Most API endpoints require JWT authentication:

```http
Authorization: Bearer <your_jwt_token>
```

### Get Token

```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "your_password"
}
```

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 86400
}
```

---

## Health Check

### Liveness Check

```http
GET /health/live
```

**Response:**
```json
{
  "status": "alive"
}
```

### Readiness Check

```http
GET /health/ready
```

**Response:**
```json
{
  "status": "ready",
  "checks": {
    "database": "ok",
    "channels": "ok"
  }
}
```

---

## WebSocket API

### Connection

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');
```

### Message Format

```json
{
  "type": "message",
  "channel": "telegram",
  "user_id": "123456789",
  "content": {
    "text": "Hello!"
  },
  "metadata": {}
}
```

### Events

| Event | Description |
|-------|-------------|
| `message` | New message received |
| `typing` | User is typing |
| `read` | Message read confirmation |
| `error` | Error occurred |

---

## HTTP REST API

### Send Message

```http
POST /api/v1/messages
Authorization: Bearer <token>
Content-Type: application/json

{
  "channel": "telegram",
  "user_id": "123456789",
  "content": {
    "text": "Hello from uHorse!"
  }
}
```

**Response:**
```json
{
  "id": "msg_abc123",
  "status": "sent",
  "timestamp": "2025-03-04T10:00:00Z"
}
```

### Get Channels

```http
GET /api/v1/channels
Authorization: Bearer <token>
```

**Response:**
```json
{
  "channels": [
    {
      "type": "telegram",
      "status": "running",
      "connected": true
    },
    {
      "type": "slack",
      "status": "stopped",
      "connected": false
    }
  ]
}
```

### Get Metrics

```http
GET /metrics
```

**Response:**
```
# HELP uhorse_messages_total Total messages processed
# TYPE uhorse_messages_total counter
uhorse_messages_total{channel="telegram"} 1234
uhorse_messages_total{channel="slack"} 567
```

---

## Tool Calling

### Execute Tool

```http
POST /api/v1/tools/execute
Authorization: Bearer <token>
Content-Type: application/json

{
  "tool": "calculator",
  "input": {
    "expression": "2 + 2 * 3"
  }
}
```

**Response:**
```json
{
  "result": 8,
  "execution_time_ms": 5
}
```

### List Tools

```http
GET /api/v1/tools
Authorization: Bearer <token>
```

**Response:**
```json
{
  "tools": [
    {
      "name": "calculator",
      "description": "Perform mathematical calculations",
      "input_schema": {...}
    },
    {
      "name": "weather",
      "description": "Get weather information",
      "input_schema": {...}
    }
  ]
}
```

---

## Error Handling

### Error Response Format

```json
{
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Missing required field: channel",
    "details": {
      "field": "channel",
      "expected": "string"
    }
  }
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `UNAUTHORIZED` | 401 | Invalid or missing token |
| `FORBIDDEN` | 403 | Permission denied |
| `NOT_FOUND` | 404 | Resource not found |
| `INVALID_REQUEST` | 400 | Invalid request body |
| `CHANNEL_ERROR` | 500 | Channel communication error |
| `RATE_LIMITED` | 429 | Too many requests |

---

## Usage Examples

### cURL

```bash
# Health check
curl http://localhost:8080/health/live

# Send message
curl -X POST http://localhost:8080/api/v1/messages \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"channel":"telegram","user_id":"123","content":{"text":"Hello!"}}'
```

### Python

```python
import requests

BASE_URL = "http://localhost:8080"
TOKEN = "your_token"

headers = {"Authorization": f"Bearer {TOKEN}"}

# Send message
response = requests.post(
    f"{BASE_URL}/api/v1/messages",
    headers=headers,
    json={
        "channel": "telegram",
        "user_id": "123456789",
        "content": {"text": "Hello!"}
    }
)
print(response.json())
```

### JavaScript

```javascript
const BASE_URL = 'http://localhost:8080';
const TOKEN = 'your_token';

// Send message
async function sendMessage(channel, userId, text) {
  const response = await fetch(`${BASE_URL}/api/v1/messages`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${TOKEN}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      channel,
      user_id: userId,
      content: { text }
    })
  });
  return response.json();
}
```
