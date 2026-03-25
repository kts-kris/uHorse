# uHorse Configuration Wizard Guide

## Overview

uHorse provides an interactive configuration wizard to help you create a starter configuration without editing files by hand.

> Note: this document describes the legacy monolithic `uhorse` wizard entry that still exists in the repository. It is not the default flow for the current Hub + Node mainline. For the current mainline, prefer `LOCAL_SETUP.md`, `TESTING.md`, and `README.md`. The examples below are normalized to the current repository mainline defaults where applicable: port `8765` and health path `/api/health`. This wizard also does not cover Node Desktop local preferences such as `notifications_enabled`, `show_notification_details`, `mirror_notifications_to_dingtalk`, or `launch_at_login`.

## Launch Configuration Wizard

### Build Project

```bash
cargo build --release
```

### Run Configuration Wizard

```bash
# Run the wizard in the current directory
./target/release/uhorse wizard

# Run the wizard in a specified directory
./target/release/uhorse wizard -d /path/to/project
```

## Wizard Flow

The current implementation walks through these steps:

1. Server configuration
2. Database configuration
3. One optional channel setup per run
4. Optional LLM configuration
5. Security configuration
6. Validation
7. Save `config.toml` and `.env`
8. Print next steps

### 1. Server Configuration

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📡 Server Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Host [127.0.0.1]:
Port [8765]:

Server configuration:
  Host: 127.0.0.1
  Port: 8765

Is this correct?
  1. Confirm
  2. Reconfigure
Select [1-2]:
```

### 2. Database Configuration

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 Database Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Select database type:
  1. SQLite (recommended)
  2. PostgreSQL
Select [1-2]:
```

#### SQLite

```text
Database file path [./data/uhorse.db]:
```

#### PostgreSQL

```text
Connection URL [postgresql://uhorse:password@localhost:5432/uhorse]:
```

After input, the wizard prints the chosen database settings and lets you confirm or reconfigure them.

### 3. Channel Configuration

The current implementation supports `Telegram`, `Slack`, `Discord`, `WhatsApp`, `DingTalk`, `Feishu`, and `WeWork`.

> Current limitation: the prompt presents a numbered menu and configures one selected channel per wizard run. If you need multiple channels, use the generated `config.toml` as a starting point and add the additional channel sections manually before starting the service.

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📱 Channel Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Available channels:
  1. Telegram ⭐
  2. Slack
  3. Discord
  4. WhatsApp
  5. DingTalk ⭐
  6. Feishu
  7. WeWork
  8. Continue (skip channel setup)
```

Examples of the current prompts:

#### Telegram

```text
Enable Telegram?
  1. Yes
  2. No
Select [1-2]:

Enter Bot Token:
Enter Webhook Secret (optional):
```

#### Slack

```text
Enable Slack?
  1. Yes
  2. No
Select [1-2]:

Enter Bot Token:
Enter Signing Secret:
```

#### Discord

```text
Enable Discord?
  1. Yes
  2. No
Select [1-2]:

Enter Bot Token:
Enter Application ID:
```

#### DingTalk

```text
Enable DingTalk?
  1. Yes
  2. No
Select [1-2]:

Enter App Key:
Enter App Secret:
Enter Agent ID:
```

> Current limitation: this step only collects DingTalk app credentials. It does not generate `channels.dingtalk.notification_bindings`. To complete the “node notification -> DingTalk user” path, you still need to add the `node_id` to `user_id` mapping manually in the Hub config.

#### Feishu

```text
Enable Feishu?
  1. Yes
  2. No
Select [1-2]:

Enter App ID:
Enter App Secret:
Enter Encrypt Key (optional):
Enter Verify Token (optional):
```

#### WeWork

```text
Enable WeWork?
  1. Yes
  2. No
Select [1-2]:

Enter Corp ID:
Enter Secret:
Enter Agent ID:
Enter Token (optional):
Enter Encoding AES Key (optional):
```

### 4. LLM Configuration

After channel setup, the wizard enters an optional LLM step. The current providers are:

- `OpenAI`
- `Azure OpenAI`
- `Anthropic (Claude)`
- `Google Gemini`
- `Custom (OpenAI-compatible)`

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🤖 LLM Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Enable LLM features?
  1. Enable
  2. Skip

Select LLM provider:
  1. OpenAI
  2. Azure OpenAI
  3. Anthropic (Claude)
  4. Google Gemini
  5. Custom (OpenAI-compatible)
```

Typical follow-up prompts:

```text
Enter API Key:
Model name [gpt-3.5-turbo]:
Temperature [0.7]:
Max Tokens [2000]:
```

Provider-specific defaults in the current implementation:

- `OpenAI` → base URL `https://api.openai.com/v1`, default model `gpt-3.5-turbo`
- `Azure OpenAI` → asks for Azure endpoint and builds a deployment base URL, default model `gpt-35-turbo`
- `Anthropic (Claude)` → base URL `https://api.anthropic.com/v1`, default model `claude-3-sonnet-20240229`
- `Google Gemini` → base URL `https://generativelanguage.googleapis.com/v1beta`, default model `gemini-pro`
- `Custom (OpenAI-compatible)` → asks for a custom API base URL

### 5. Security Configuration

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🔒 Security Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

JWT secret is used to sign access tokens.
Use at least 32 random characters.

Generate a secure JWT secret automatically?
  1. Auto-generate
  2. Enter manually
Select [1-2]:

Token expiry in seconds [86400]:
```

### 6. Validation

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ✓ Configuration Validation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ Configuration validated
```

Notes about the current validation logic:

- Ports below `1024` are rejected.
- SQLite parent directories are created automatically if they do not exist.
- If the JWT secret is shorter than `32` characters, the wizard warns and asks whether to continue.

### 7. Save Configuration

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 Save Configuration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ Configuration saved to: ./config.toml
✓ Environment variables saved to: ./.env
```

### 8. Completion

```text
╔════════════════════════════════════════════════╗
║                                                ║
║     🎉 Configuration Complete!                 ║
║                                                ║
╚════════════════════════════════════════════════╝

Next steps:
  1. Start uHorse:
     ./start.sh

  2. Check service health:
     curl http://127.0.0.1:8765/api/health

> Additional note: the current Hub also exposes the standard Prometheus metrics endpoint at `/metrics`, but that is outside the scope of this legacy wizard.
```

## Command Line Options

```bash
uhorse wizard --help

Options:
  -d, --dir <PATH>    Target directory (default: current)
  -h, --help          Show help message
```

The current wizard only supports `-d/--dir` and `-h/--help`.

## Generated Files

The wizard writes both `config.toml` and `.env` into the target directory. It overwrites existing files in that directory.

The current implementation does not read prompt defaults from environment variables.

### Generated `.env`

```bash
# uHorse environment variables
# Generated by the configuration wizard

UHORSE_SERVER_HOST=127.0.0.1
UHORSE_SERVER_PORT=8765
UHORSE_TELEGRAM_BOT_TOKEN=YOUR_BOT_TOKEN
RUST_LOG=info
```

### Generated `config.toml`

A typical generated file looks like this:

```toml
# uHorse configuration file
# Generated by the configuration wizard

[server]
host = "127.0.0.1"
port = 8765

[channels]
enabled = ["telegram"]

[channels.telegram]
bot_token = "123456789:ABC..."

[database]
path = "./data/uhorse.db"

[security]
jwt_secret = "YOUR_JWT_SECRET"
token_expiry = 86400

[llm]
enabled = true
provider = "openai"
api_key = "sk-..."
base_url = "https://api.openai.com/v1"
model = "gpt-3.5-turbo"
temperature = 0.7
max_tokens = 2000
```

## Next Steps

After configuration:

```bash
# Start with the repository helper
./start.sh

# Verify health
curl http://127.0.0.1:8765/api/health
```

## Troubleshooting

### Permission Denied

```bash
chmod +x ./target/release/uhorse
```

### Existing Config Will Be Overwritten

Back up the target files before running the wizard again:

```bash
cp config.toml config.toml.bak
cp .env .env.bak
```

### `openssl` Is Missing

Automatic JWT secret generation uses `openssl rand -hex 32`.

If `openssl` is unavailable:

- install `openssl`, or
- choose manual JWT secret input in the wizard

### Port Below 1024 Fails Validation

The current validation rejects privileged ports below `1024`. Use a higher port such as `8765`.

### Multi-channel Setup

The current prompt configures one channel choice per run. For multi-channel setups, use the generated file as a base and add the extra channel sections manually in `config.toml`.
