# uHorse Configuration Guide

## Table of Contents

- [Quick Start](#quick-start)
- [Configuration Files](#configuration-files)
- [Environment Variables](#environment-variables)
- [Module Configuration](#module-configuration)
- [Channel Configuration](#channel-configuration)
- [LLM Configuration](#llm-configuration)
- [API Configuration](#api-configuration)
- [Production Configuration](#production-configuration)

---

## Quick Start

### 1. Minimal Configuration

Create `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = []

[database]
path = "./data/uhorse.db"
```

### 2. Run with Configuration

```bash
uhorse run --config config.toml
```

---

## Configuration Files

### File Locations

uHorse looks for configuration files in the following order:

1. `--config <path>` command line argument
2. `UHORSE_CONFIG` environment variable
3. `./config.toml` current directory
4. `~/.uhorse/config.toml` user directory
5. `/etc/uhorse/config.toml` system directory

### Configuration Format

Supports TOML, JSON, and YAML formats:

```
config.toml    # TOML (recommended)
config.json    # JSON
config.yaml    # YAML
```

---

## Environment Variables

### Server

| Variable | Description | Default |
|----------|-------------|---------|
| `UHORSE_HOST` | Server host | `127.0.0.1` |
| `UHORSE_PORT` | Server port | `8080` |
| `UHORSE_CONFIG` | Config file path | - |

### Database

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | Database URL | - |
| `UHORSE_DB_PATH` | SQLite path | `./data/uhorse.db` |

### LLM

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `LLM_PROVIDER` | LLM provider | `openai` |
| `LLM_MODEL` | Model name | `gpt-4` |

---

## Module Configuration

### Server Configuration

```toml
[server]
host = "0.0.0.0"           # Listen address
port = 8080                 # Listen port
workers = 4                 # Worker threads
graceful_shutdown = true    # Graceful shutdown

[server.tls]
enabled = false
cert_path = "./certs/cert.pem"
key_path = "./certs/key.pem"
```

### Database Configuration

```toml
[database]
type = "sqlite"             # sqlite or postgres
path = "./data/uhorse.db"   # SQLite path

# For PostgreSQL
# [database]
# type = "postgres"
# url = "postgresql://user:pass@localhost/uhorse"
# pool_size = 10
```

### Logging Configuration

```toml
[logging]
level = "info"              # trace, debug, info, warn, error
format = "json"             # json or text
output = "stdout"           # stdout or file path

[logging.file]
enabled = false
path = "./logs/uhorse.log"
max_size = "100MB"
max_files = 10
```

---

## Channel Configuration

### Telegram

```toml
[channels.telegram]
enabled = true
bot_token = "your_bot_token"
webhook_url = "https://your-domain.com/webhook/telegram"
webhook_secret = "optional_secret"

[channels.telegram.rate_limit]
messages_per_second = 30
messages_per_minute = 500
```

### Slack

```toml
[channels.slack]
enabled = true
bot_token = "xoxb-your-bot-token"
app_token = "xapp-your-app-token"
signing_secret = "your_signing_secret"
```

### Discord

```toml
[channels.discord]
enabled = true
bot_token = "your_discord_bot_token"
application_id = "123456789"

[channels.discord.intents]
guilds = true
guild_messages = true
direct_messages = true
```

### DingTalk

```toml
[channels.dingtalk]
enabled = true
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789
```

### Feishu

```toml
[channels.feishu]
enabled = true
app_id = "your_app_id"
app_secret = "your_app_secret"
```

---

## LLM Configuration

### OpenAI

```toml
[llm]
provider = "openai"

[llm.openai]
api_key = "sk-..."
model = "gpt-4"
base_url = "https://api.openai.com/v1"  # Optional
temperature = 0.7
max_tokens = 4096
```

### Anthropic

```toml
[llm]
provider = "anthropic"

[llm.anthropic]
api_key = "sk-ant-..."
model = "claude-3-opus-20240229"
max_tokens = 4096
```

### Multiple Providers

```toml
[llm]
default_provider = "openai"

[llm.providers.openai]
api_key = "sk-..."
model = "gpt-4"

[llm.providers.anthropic]
api_key = "sk-ant-..."
model = "claude-3-opus"

[llm.providers.gemini]
api_key = "..."
model = "gemini-pro"
```

---

## API Configuration

```toml
[api]
prefix = "/api/v1"          # API path prefix
rate_limit = 100            # Requests per minute
timeout = 30                # Request timeout (seconds)

[api.cors]
enabled = true
origins = ["*"]
methods = ["GET", "POST", "PUT", "DELETE"]
headers = ["Authorization", "Content-Type"]

[api.auth]
enabled = true
jwt_secret = "your-secret-key"
token_expiry = "24h"
refresh_token_expiry = "7d"
```

---

## Production Configuration

### Security Settings

```toml
[security]
device_pairing = true       # Require device approval
approval_workflow = true    # Sensitive operation approval
audit_log = true            # Enable audit logging

[security.rate_limit]
enabled = true
requests_per_minute = 60
burst = 10
```

### Performance Tuning

```toml
[performance]
max_connections = 10000
connection_timeout = 60
keep_alive = true

[performance.cache]
enabled = true
ttl = 300                   # Cache TTL (seconds)
max_size = "100MB"
```

### Observability

```toml
[observability.tracing]
enabled = true
endpoint = "http://localhost:4317"
sample_rate = 0.1           # 10% sampling

[observability.metrics]
enabled = true
endpoint = "/metrics"
port = 9090

[observability.audit]
enabled = true
storage = "database"        # database or file
retention = "90d"
```

---

## Full Example

```toml
# config.toml - Complete Configuration Example

[server]
host = "0.0.0.0"
port = 8080

[database]
type = "sqlite"
path = "./data/uhorse.db"

[channels]
enabled = ["telegram", "slack"]

[channels.telegram]
bot_token = "${TELEGRAM_BOT_TOKEN}"  # Environment variable

[channels.slack]
bot_token = "${SLACK_BOT_TOKEN}"

[llm]
provider = "openai"

[llm.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4"

[api.auth]
jwt_secret = "${JWT_SECRET}"

[security]
audit_log = true
```
