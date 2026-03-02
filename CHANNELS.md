# uHorse 通道集成指南

## 目录

- [通道概述](#通道概述)
- [Telegram Bot](#telegram-bot)
- [Slack](#slack)
- [Discord](#discord)
- [WhatsApp Business](#whatsapp-business)
- [Webhook 配置](#webhook-配置)
- [消息格式](#消息格式)
- [测试验证](#测试验证)

---

## 通道概述

uHorse 支持多通道消息发送和接收：

| 通道 | 类型 | 用途 |
|------|------|------|
| Telegram | Bot API | 个人用户聊天 |
| Slack | Events API | 团队协作 |
| Discord | Bot API | 社区管理 |
| WhatsApp | Business API | 客户服务 |

---

## Telegram Bot

### 1. 创建 Bot

1. 在 Telegram 中搜索 [@BotFather](https://t.me/botfather)
2. 发送 `/newbot` 创建新 Bot
3. 按提示设置 Bot 名称和用户名
4. 获得 Bot Token：`123456789:ABCdefGHIjklMNOpqrsTUVwxyz`

### 2. 配置 uHorse

**config.toml:**
```toml
[channels.telegram]
bot_token = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
```

**或使用环境变量:**
```bash
export UHORSE_TELEGRAM_BOT_TOKEN="123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
```

### 3. 设置 Webhook

```bash
curl -X POST "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/setWebhook" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://your-domain.com/api/v1/channels/telegram/webhook",
    "secret_token": "your_webhook_secret"
  }'
```

### 4. 测试 Bot

```bash
# 发送测试消息
curl -X POST "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/sendMessage" \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": "YOUR_CHAT_ID",
    "text": "Hello from uHorse!"
  }'
```

### 5. 接收消息

用户发送给 Bot 的消息会通过 Webhook 转发到 uHorse。

---

## Slack

### 1. 创建 Slack App

1. 访问 https://api.slack.com/apps
2. 点击 "Create New App"
3. 填写 App 名称和选择工作区
4. 创建 Bot

### 2. 配置权限

**Bot Token Scopes:**
- `chat:write` - 发送消息
- `channels:read` - 读取频道
- `groups:read` - 读取私聊
- `im:write` - 发送私信
- `mpim:write` - 发送群私聊

**Event Subscriptions:**
- `message.channels` - 频道消息
- `message.groups` - 私聊消息
- `message.im` - IM 消息
- `message.mpim` - 群私聊消息

### 3. 安装到工作区

1. 在左侧菜单选择 "Install App"
2. 选择要安装的工作区
3. 点击 "Install"

### 4. 获取凭证

1. **OAuth Token**: 在 "Basic Information" 中
2. **Bot Token**: 在 "OAuth & Permissions" → "Bot Tokens" 中
3. **Signing Secret**: 在 "Basic Information" → "App Credentials" 中

### 5. 配置 uHorse

**config.toml:**
```toml
[channels.slack]
bot_token = "xoxb-YOUR-BOT-TOKEN"
signing_secret = "YOUR_SIGNING_SECRET"
```

**或使用环境变量:**
```bash
export UHORSE_SLACK_BOT_TOKEN="xoxb-YOUR-BOT-TOKEN"
export UHORSE_SLACK_SIGNING_SECRET="YOUR_SIGNING_SECRET"
```

### 6. 配置事件订阅

```bash
curl -X POST https://slack.com/api/methods.subscriptions.list \
  -H "Authorization: Bearer xoxb-YOUR-TOKEN"
```

---

## Discord

### 1. 创建 Discord Application

1. 访问 https://discord.com/developers/applications
2. 点击 "New Application"
3. 填写应用名称
4. 创建应用

### 2. 创建 Bot

1. 在左侧菜单选择 "Bot"
2. 点击 "Add Bot"
3. 设置 Bot 用户名和头像
4. 保存获得 Bot Token

### 3. 配置 Intents

启用以下 Privileged Gateway Intents：
- `SERVER_CONTENT` - 服务器内容
- `MESSAGE_CONTENT` - 消息内容

### 4. 邀请 Bot 到服务器

生成 OAuth2 URL：
1. 在 "OAuth2" → "URL Generator"
2. 选择 scopes:
   - `bot`
   - `applications.commands`
3. 选择 bot permissions:
   - Send Messages
   - Read Messages/View Channels
   - Read Message History
4. 生成 URL 并访问
5. 选择服务器授权

### 5. 配置 uHorse

**config.toml:**
```toml
[channels.discord]
bot_token = "MTIzNDU2Nzg5MA.Gh4b2.example"
application_id = "123456789012345678"
```

**或使用环境变量:**
```bash
export UHORSE_DISCORD_BOT_TOKEN="MTIzNDU2Nzg5MA..."
export UHORSE_DISCORD_APPLICATION_ID="123456789012345678"
```

---

## WhatsApp Business API

### 1. 创建 Meta App

1. 访问 https://developers.facebook.com/apps
2. 点击 "Create App"
3. 选择 "Business" 类型
4. 填写应用名称

### 2. 添加 WhatsApp 产品

1. 在应用配置中选择 "Add Product"
2. 选择 "WhatsApp"
3. 配置 WhatsApp API 设置

### 3. 配置 Webhook

1. 在 WhatsApp → Configuration
2. 设置 Webhook URL: `https://your-domain.com/api/v1/channels/whatsapp/webhook`
3. 设置 Verify Token
4. 订阅消息字段

### 4. 获取凭证

1. **Access Token**: 在 WhatsApp → API Setup 中生成
2. **Phone Number ID**: 在发送方号码中查看
3. **Business Account ID**: 在 WhatsApp Manager 中查看

### 5. 配置 uHorse

**config.toml:**
```toml
[channels.whatsapp]
access_token = "YOUR_ACCESS_TOKEN"
phone_number_id = "YOUR_PHONE_NUMBER_ID"
business_account_id = "YOUR_BUSINESS_ACCOUNT_ID"
webhook_verify_token = "YOUR_VERIFY_TOKEN"
```

**或使用环境变量:**
```bash
export UHORSE_WHATSAPP_ACCESS_TOKEN="..."
export UHORSE_WHATSAPP_PHONE_NUMBER_ID="..."
export UHORSE_WHATSAPP_BUSINESS_ACCOUNT_ID="..."
```

---

## Webhook 配置

### 本地开发 Webhook

使用 ngrok 暴露本地服务：

```bash
# 安装 ngrok
brew install ngrok  # macOS
# 或从 https://ngrok.com 下载

# 启动 ngrok
ngrok http 8080

# 获得公网 URL
# 例如: https://abc123.ngrok.io
```

配置 Webhook URL：
```bash
# Telegram
curl -X POST "https://api.telegram.org/bot<TOKEN>/setWebhook" \
  -d '{
    "url": "https://abc123.ngrok.io/api/v1/channels/telegram/webhook"
  }'
```

### 生产环境 Webhook

```bash
# 使用 Nginx 反向代理
location /api/v1/channels/ {
    proxy_pass http://localhost:8080/api/v1/channels/;
}
```

---

## 消息格式

### 文本消息

```json
{
  "type": "message",
  "channel": "telegram",
  "data": {
    "chat_id": "123456789",
    "content": {
      "type": "text",
      "text": "Hello from uHorse!"
    }
  }
}
```

### 图片消息

```json
{
  "type": "message",
  "channel": "telegram",
  "data": {
    "chat_id": "123456789",
    "content": {
      "type": "image",
      "url": "https://example.com/image.jpg",
      "caption": "Image description"
    }
  }
}
```

### 音频消息

```json
{
  "type": "message",
  "channel": "telegram",
  "data": {
    "chat_id": "123456789",
    "content": {
      "type": "audio",
      "url": "https://example.com/audio.mp3"
    }
  }
}
```

---

## 测试验证

### Telegram 测试

```bash
# 发送文本消息
curl -X POST http://localhost:8080/api/v1/channels/telegram/send \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": "YOUR_CHAT_ID",
    "text": "Hello from uHorse!"
  }'
```

### Slack 测试

```bash
# 发送消息到频道
curl -X POST http://localhost:8080/api/v1/channels/slack/send \
  -H "Content-Type: application/json" \
  -d '{
    "channel": "#general",
    "text": "Hello from uHorse!"
  }'
```

### Discord 测试

```bash
# 发送消息到频道
curl -X POST http://localhost:8080/api/v1/channels/discord/send \
  -H "Content-Type: application/json" \
  -d '{
    "channel_id": "CHANNEL_ID",
    "content": "Hello from uHorse!"
  }'
```

### WhatsApp 测试

```bash
# 发送消息
curl -X POST http://localhost:8080/api/v1/channels/whatsapp/send \
  -H "Content-Type: application/json" \
  -d '{
    "to": "1234567890",
    "type": "text",
    "text": "Hello from uHorse!"
  }'
```

---

## 通道状态监控

### 查看所有通道状态

```bash
curl http://localhost:8080/api/v1/channels/status
```

**响应:**
```json
{
  "channels": {
    "telegram": {
      "connected": true,
      "webhook": "active",
      "last_message": "2026-03-02T12:00:00Z"
    },
    "slack": {
      "connected": true,
      "webhook": "active",
      "last_message": "2026-03-02T11:55:00Z"
    },
    "discord": {
      "connected": false,
      "error": "Bot token not configured"
    },
    "whatsapp": {
      "connected": false,
      "error": "Access token expired"
    }
  }
}
```

---

## 下一步

- [配置指南](CONFIG.md)
- [API 使用指南](API.md)
- [部署指南](deployments/DEPLOYMENT.md)
