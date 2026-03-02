# uHorse 配置向导使用指南

## 概述

uHorse 提供交互式配置向导，帮助您快速完成项目配置，无需手动编辑配置文件。

## 启动配置向导

### 编译项目

```bash
cargo build --release
```

### 运行配置向导

```bash
# 在当前目录运行配置向导
./target/release/uhorse wizard

# 在指定目录运行配置向导
./target/release/uhorse wizard -d /path/to/project
```

## 配置向导流程

配置向导将引导您完成以下步骤：

### 1. 服务器配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📡 服务器配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

监听地址 [127.0.0.1]:
监听端口 [8080]:

服务器配置:
  监听地址: 127.0.0.1
  监听端口: 8080

是否正确?
  1. 确认
  2. 重新配置
请选择 [1-2]:
```

**配置项说明：**
- **监听地址**: 服务器绑定的 IP 地址
  - `127.0.0.1` - 仅本地访问（推荐用于开发）
  - `0.0.0.0` - 允许所有网络接口访问（生产环境）
- **监听端口**: 服务器监听端口（默认 8080）

### 2. 数据库配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 数据库配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

选择数据库类型:
  1. SQLite (推荐)
  2. PostgreSQL
请选择 [1-2]:
```

#### SQLite 配置（推荐）

```
数据库文件路径 [./data/uhorse.db]:
```

**SQLite 特点：**
- ✅ 零配置，开箱即用
- ✅ 轻量级，适合小型部署
- ✅ 无需额外服务
- ⚠️ 不支持高并发写入

#### PostgreSQL 配置

```
连接 URL [postgresql://uhorse:password@localhost:5432/uhorse]:
```

**PostgreSQL 特点：**
- ✅ 支持高并发
- ✅ 适合生产环境
- ✅ 完整的 ACID 支持
- ⚠️ 需要单独的数据库服务

### 3. 通道配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📱 通道配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

选择要启用的通道:

  1. Telegram
  2. Slack
  3. Discord
  4. WhatsApp

选择要配置的通道 (输入序号，多个用空格分隔):
  1. 继续 (跳过通道配置)
  2. 1
  3. 2
  4. 3
  5. 4
请选择 [1-5]:
```

#### Telegram 配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Telegram 配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

是否启用 Telegram?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
```

**获取 Telegram Bot Token：**
1. 在 Telegram 中搜索 [@BotFather](https://t.me/botfather)
2. 发送 `/newbot` 创建新 Bot
3. 按提示设置 Bot 名称和用户名
4. 获得 Bot Token（格式：`123456789:ABCdefGHIjklMNOpqrsTUVwxyz`）

#### Slack 配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Slack 配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

是否启用 Slack?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
请输入 Signing Secret:
```

**获取 Slack 凭证：**
1. 访问 https://api.slack.com/apps
2. 创建新 App
3. 配置 Bot Token Scopes 和 OAuth Scopes
4. 安装到工作区

#### Discord 配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Discord 配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

是否启用 Discord?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
请输入 Application ID:
```

**获取 Discord 凭证：**
1. 访问 https://discord.com/developers/applications
2. 创建 Application
3. 创建 Bot 并获取 Token
4. 在 OAuth2 中生成邀请 URL

#### WhatsApp 配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  WhatsApp 配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

是否启用 WhatsApp?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Phone Number ID:
请输入 Business Account ID:
请输入 Webhook Verify Token:
```

**获取 WhatsApp 凭证：**
1. 访问 https://developers.facebook.com/apps
2. 创建 Meta App
3. 添加 WhatsApp 产品
4. 配置 Webhook 并获取凭证

### 4. 安全配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🔒 安全配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

JWT 密钥用于签名访问令牌。
请使用至少 32 个随机字符。

是否自动生成安全的 JWT 密钥?
  1. 自动生成
  2. 手动输入
请选择 [1-2]:
```

**JWT 密钥说明：**
- 用于签名访问令牌
- 至少 32 个随机字符
- 建议使用自动生成

```
访问令牌过期时间（秒）[86400]:
```

**令牌过期时间说明：**
- 默认 86400 秒（24 小时）
- 可根据安全需求调整
- 较短的过期时间更安全

### 5. 配置验证

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ✓ 配置验证
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ 配置验证通过
```

### 6. 保存配置

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 保存配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ 配置已保存到: ./config.toml
✓ 环境变量已保存到: ./.env
```

### 7. 完成

```
╔════════════════════════════════════════════════╗
║                                                ║
║     🎉 配置完成！                             ║
║                                                ║
╚════════════════════════════════════════════════╝

下一步操作:

  1️⃣  启动 uHorse:
     ./start.sh

  2️⃣  查看服务状态:
     curl http://127.0.0.1:8080/health/live

  3️⃣  配置通道 Webhook:
     请参考 CHANNELS.md 配置各通道的 Webhook URL

  4️⃣  查看配置:
     cat config.toml

📚 文档:
  - 配置指南: CONFIG.md
  - API 使用: API.md
  - 通道集成: CHANNELS.md

💡 提示:
  - 配置文件已保存到项目根目录
  - 可随时编辑 config.toml 或 .env 修改配置
  - 重新运行向导: ./target/release/uhorse wizard
```

## 配置文件说明

配置向导会生成两个文件：

### config.toml

主配置文件，包含所有配置项：

```toml
# uHorse 配置文件
# 由配置向导生成

[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = ["telegram"]

[channels.telegram]
bot_token = "YOUR_BOT_TOKEN"

[database]
path = "./data/uhorse.db"

[security]
jwt_secret = "YOUR_JWT_SECRET"
token_expiry = 86400
```

### .env

环境变量文件，用于覆盖配置：

```bash
# uHorse 环境变量
# 由配置向导生成

UHORSE_SERVER_HOST=127.0.0.1
UHORSE_SERVER_PORT=8080
UHORSE_TELEGRAM_BOT_TOKEN=YOUR_BOT_TOKEN
RUST_LOG=info
```

## 常见问题

### Q: 如何重新配置？

```bash
# 重新运行配置向导
./target/release/uhorse wizard
```

### Q: 如何修改已有配置？

有两种方式：

**方式一：重新运行配置向导**
```bash
./target/release/uhorse wizard
```

**方式二：手动编辑配置文件**
```bash
# 编辑主配置文件
vi config.toml

# 或编辑环境变量文件
vi .env
```

### Q: 配置向导会覆盖现有配置吗？

是的，配置向导会覆盖现有的 `config.toml` 和 `.env` 文件。如果需要保留现有配置，请先备份：

```bash
cp config.toml config.toml.bak
cp .env .env.bak
```

### Q: 如何只配置部分通道？

在通道配置步骤中，只输入需要配置的通道序号即可：

```
选择要配置的通道 (输入序号，多个用空格分隔): 1 3
```

这只会配置 Telegram（1）和 Discord（3）。

### Q: 数据库配置支持其他类型吗？

目前配置向导只支持 SQLite 和 PostgreSQL。如需使用其他数据库（如 MySQL），请手动编辑 `config.toml`。

### Q: 生成的 JWT 密钥安全吗？

配置向导使用 `openssl rand -hex 32` 生成 32 字节的随机密钥，非常安全。请确保生成的密钥不被泄露。

## 高级用法

### 在非交互模式下使用

配置向导目前只支持交互模式。如需自动化配置，请：

1. 使用模板配置文件
2. 通过脚本修改配置
3. 或等待后续版本的自动化配置功能

### 配置模板

创建配置模板：

```bash
# 使用默认配置启动
./target/release/uhorse -c config.toml run
```

### 多环境配置

为不同环境创建不同的配置文件：

```bash
# 开发环境
./target/release/uhorse -c config.dev.toml run

# 生产环境
./target/release/uhorse -c config.prod.toml run
```

## 相关文档

- [配置指南](CONFIG.md) - 完整配置说明
- [API 使用指南](API.md) - API 文档和使用示例
- [通道集成指南](CHANNELS.md) - 各通道集成步骤
- [部署指南](deployments/DEPLOYMENT.md) - 生产环境部署
