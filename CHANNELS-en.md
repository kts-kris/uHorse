# uHorse Channel Integration Guide

## Table of Contents

- [Channel Overview](#channel-overview)
- [Telegram Bot](#telegram-bot)
- [Slack](#slack)
- [Discord](#discord)
- [WhatsApp Business](#whatsapp-business)
- [DingTalk](#dingtalk)
- [Feishu](#feishu)
- [Webhook Configuration](#webhook-configuration)
- [Message Format](#message-format)
- [Testing & Verification](#testing--verification)

---

## Channel Overview

uHorse supports multi-channel message sending and receiving:

| Channel | Type | Use Case |
|---------|------|----------|
| Telegram | Bot API | Personal user chat |
| Slack | Events API | Team collaboration |
| Discord | Bot API | Community management |
| WhatsApp | Business API | Customer service |
| DingTalk | Enterprise API | Enterprise communication |
| Feishu | Enterprise API | Enterprise collaboration |
| WeCom | Enterprise API | Internal enterprise |

### Enable Channels

```toml
# config.toml
[channels]
enabled = ["telegram", "slack", "discord"]
```

---

## Telegram Bot

### 1. Create Bot

1. Open Telegram, search for [@BotFather](https://t.me/botfather)
2. Send `/newbot` command
3. Follow prompts to set bot name
4. Save the returned bot token

### 2. Configure

```toml
[channels.telegram]
enabled = true
bot_token = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
webhook_url = "https://your-domain.com/webhook/telegram"
```

### 3. Webhook Setup

```bash
# Set webhook
curl -X POST "https://api.telegram.org/bot<YOUR_BOT_TOKEN>/setWebhook" \
  -H "Content-Type: application/json" \
  -d '{"url": "https://your-domain.com/webhook/telegram"}'
```

### 4. Features

- Text messages
- Images and files
- Inline keyboards
- Callback queries
- Group chat support

---

## Slack

### 1. Create Slack App

1. Go to [Slack API](https://api.slack.com/apps)
2. Click "Create New App"
3. Select "From scratch"
4. Enter app name and workspace

### 2. Configure Permissions

Required OAuth scopes:
- `app_mentions:read` - Read @mentions
- `chat:write` - Send messages
- `channels:history` - Read channel history
- `im:history` - Read direct messages

### 3. Configure

```toml
[channels.slack]
enabled = true
bot_token = "xoxb-your-bot-token"
app_token = "xapp-your-app-token"
signing_secret = "your-signing-secret"
```

### 4. Enable Events

Subscribe to events:
- `app_mention` - Bot mentioned
- `message.im` - Direct message
- `message.channels` - Channel message

---

## Discord

### 1. Create Discord Application

1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Click "New Application"
3. Enter application name
4. Navigate to "Bot" section
5. Click "Add Bot"

### 2. Configure Permissions

Required permissions:
- Read Messages
- Send Messages
- Read Message History
- Embed Links
- Attach Files

### 3. Configure

```toml
[channels.discord]
enabled = true
bot_token = "your-discord-bot-token"
application_id = "123456789012345678"

[channels.discord.intents]
guilds = true
guild_messages = true
direct_messages = true
message_content = true
```

### 4. Invite Bot

```
https://discord.com/api/oauth2/authorize?client_id=<YOUR_CLIENT_ID>&permissions=2048&scope=bot
```

---

## WhatsApp Business

### 1. Setup WhatsApp Business API

1. Apply for [WhatsApp Business API](https://www.whatsapp.com/business/api)
2. Complete business verification
3. Get phone number ID and access token

### 2. Configure

```toml
[channels.whatsapp]
enabled = true
phone_number_id = "123456789"
access_token = "your-access-token"
verify_token = "your-verify-token"
webhook_url = "https://your-domain.com/webhook/whatsapp"
```

### 3. Webhook Verification

```bash
# Verify webhook
curl "https://graph.facebook.com/v18.0/<PHONE_NUMBER_ID>/webhooks" \
  -H "Authorization: Bearer <ACCESS_TOKEN>"
```

---

## DingTalk

### 1. Create DingTalk Application

1. Login to [DingTalk Developer Platform](https://open.dingtalk.com/)
2. Create application
3. Configure application permissions
4. Get AppKey and AppSecret

### 2. Configure

```toml
[channels.dingtalk]
enabled = true
app_key = "your-app-key"
app_secret = "your-app-secret"
agent_id = 123456789
```

### 3. Event Subscription

Subscribe to events:
- Chat message receive
- Group message receive
- User interaction events

---

## Feishu

### 1. Create Feishu Application

1. Login to [Feishu Open Platform](https://open.feishu.cn/)
2. Create enterprise self-built application
3. Configure application permissions
4. Get App ID and App Secret

### 2. Configure

```toml
[channels.feishu]
enabled = true
app_id = "cli_xxxxxxxxxx"
app_secret = "your-app-secret"
encrypt_key = "your-encrypt-key"  # Optional
verification_token = "your-token"  # Optional
```

### 3. Configure Events

Subscribe to events:
- `im.message.receive_v1` - Receive messages
- `contact.user.updated_v3` - User info updated

---

## Webhook Configuration

### Webhook URL Format

```
https://your-domain.com/webhook/<channel_type>
```

### Example

```nginx
# Nginx configuration
location /webhook/ {
    proxy_pass http://127.0.0.1:8080/webhook/;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}
```

### Webhook Security

```toml
[webhook]
verify_signature = true
allowed_ips = ["192.168.1.0/24"]
```

---

## Message Format

### Incoming Message

```json
{
  "channel": "telegram",
  "user_id": "123456789",
  "chat_id": "123456789",
  "message_id": "abc123",
  "content": {
    "type": "text",
    "text": "Hello!"
  },
  "metadata": {
    "username": "john_doe",
    "timestamp": 1709520000
  }
}
```

### Outgoing Message

```json
{
  "channel": "telegram",
  "user_id": "123456789",
  "content": {
    "type": "text",
    "text": "Hello from uHorse!"
  }
}
```

### Rich Content

```json
{
  "content": {
    "type": "image",
    "url": "https://example.com/image.png",
    "caption": "Image caption"
  }
}
```

---

## Testing & Verification

### 1. Check Channel Status

```bash
curl http://localhost:8080/api/v1/channels
```

### 2. Send Test Message

```bash
curl -X POST http://localhost:8080/api/v1/messages \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "channel": "telegram",
    "user_id": "123456789",
    "content": {"text": "Test message"}
  }'
```

### 3. View Logs

```bash
# View channel logs
tail -f logs/uhorse.log | grep channel

# View webhook requests
tail -f logs/uhorse.log | grep webhook
```

---

## Troubleshooting

### Telegram

| Problem | Solution |
|---------|----------|
| Webhook not set | Check if webhook URL is accessible |
| Bot not responding | Verify bot token is correct |
| Permission denied | Check bot permissions in group |

### Slack

| Problem | Solution |
|---------|----------|
| Events not received | Verify event subscription URL |
| Message send failed | Check OAuth token permissions |
| Signature verification failed | Verify signing secret |

### Discord

| Problem | Solution |
|---------|----------|
| Bot offline | Check bot token is valid |
| Missing intents | Enable required intents in Developer Portal |
| Permission errors | Re-invite bot with correct permissions |
