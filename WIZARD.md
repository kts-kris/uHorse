# uHorse 配置向导使用指南

## 概述

uHorse 提供交互式配置向导，帮助您生成一份可用的初始配置，无需手动从零编辑文件。

> 注意：本向导文档描述的是仓库中仍保留的旧单体 `uhorse` 配置入口，不是当前 Hub + Node 主线的默认联调路径。当前主线请优先参考 `LOCAL_SETUP.md`、`TESTING.md` 与 `README.md`。文中的示例已按当前仓库主线默认值统一到端口 `8765` 与健康检查路径 `/api/health`。本向导也不会覆盖 Node Desktop 本地偏好项，例如 `notifications_enabled`、`show_notification_details`、`mirror_notifications_to_dingtalk`、`launch_at_login`。

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

当前实现会依次执行以下步骤：

1. 服务器配置
2. 数据库配置
3. 单次运行选择 1 个可选通道进行配置
4. 可选的 LLM 配置
5. 安全配置
6. 配置验证
7. 保存 `config.toml` 与 `.env`
8. 输出后续操作提示

### 1. 服务器配置

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📡 服务器配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

监听地址 [127.0.0.1]:
监听端口 [8765]:

服务器配置:
  监听地址: 127.0.0.1
  监听端口: 8765

是否正确?
  1. 确认
  2. 重新配置
请选择 [1-2]:
```

### 2. 数据库配置

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 数据库配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

选择数据库类型:
  1. SQLite (推荐)
  2. PostgreSQL
请选择 [1-2]:
```

#### SQLite

```text
数据库文件路径 [./data/uhorse.db]:
```

#### PostgreSQL

```text
连接 URL [postgresql://uhorse:password@localhost:5432/uhorse]:
```

输入完成后，向导会打印当前数据库配置，并允许您确认或重新配置。

### 3. 通道配置

当前实现支持 `Telegram`、`Slack`、`Discord`、`WhatsApp`、`钉钉`、`飞书`、`企业微信`。

> 当前限制：通道步骤虽然展示为编号菜单，但一次运行只会配置 1 个被选中的通道。如果您需要多通道配置，建议先用向导生成初始 `config.toml`，再手动补充其余通道配置段。

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📱 通道配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

可选通道:
  1. Telegram ⭐
  2. Slack
  3. Discord
  4. WhatsApp
  5. 钉钉 ⭐
  6. 飞书
  7. 企业微信
  8. 继续（跳过通道配置）
```

当前实现中的部分提示示例如下：

#### Telegram

```text
是否启用 Telegram?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
请输入 Webhook Secret (可选):
```

#### Slack

```text
是否启用 Slack?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
请输入 Signing Secret:
```

#### Discord

```text
是否启用 Discord?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Bot Token:
请输入 Application ID:
```

#### 钉钉

```text
是否启用 钉钉?
  1. 是
  2. 否
请选择 [1-2]:

请输入 App Key:
请输入 App Secret:
请输入 Agent ID:
```

> 当前限制：这里仅收集 DingTalk App 凭据，不会自动生成 `channels.dingtalk.notification_bindings`。如果要打通“节点通知 -> 钉钉用户”链路，仍需在 Hub 配置中手工补充 `node_id` 到 `user_id` 的映射。

#### 飞书

```text
是否启用 飞书?
  1. 是
  2. 否
请选择 [1-2]:

请输入 App ID:
请输入 App Secret:
请输入 Encrypt Key (可选):
请输入 Verify Token (可选):
```

#### 企业微信

```text
是否启用 企业微信?
  1. 是
  2. 否
请选择 [1-2]:

请输入 Corp ID:
请输入 Secret:
请输入 Agent ID:
请输入 Token (可选):
请输入 Encoding AES Key (可选):
```

### 4. LLM 配置

通道配置后，向导会进入可选的 LLM 步骤。当前支持的服务商有：

- `OpenAI`
- `Azure OpenAI`
- `Anthropic (Claude)`
- `Google Gemini`
- `自定义（OpenAI 兼容）`

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🤖 大语言模型配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

是否启用大语言模型功能?
  1. 启用
  2. 跳过

选择 LLM 服务商:
  1. OpenAI
  2. Azure OpenAI
  3. Anthropic (Claude)
  4. Google Gemini
  5. 自定义 (OpenAI 兼容)
```

典型后续提示：

```text
请输入 API Key:
模型名称 [gpt-3.5-turbo]:
Temperature [0.7]:
最大 Tokens 数 [2000]:
```

当前实现中的 provider 默认值：

- `OpenAI` → `base_url = https://api.openai.com/v1`，默认模型 `gpt-3.5-turbo`
- `Azure OpenAI` → 先输入 Azure endpoint，再拼出 deployment base URL，默认模型 `gpt-35-turbo`
- `Anthropic (Claude)` → `base_url = https://api.anthropic.com/v1`，默认模型 `claude-3-sonnet-20240229`
- `Google Gemini` → `base_url = https://generativelanguage.googleapis.com/v1beta`，默认模型 `gemini-pro`
- `自定义（OpenAI 兼容）` → 手动输入 API Base URL

### 5. 安全配置

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🔒 安全配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

JWT 密钥用于签名访问令牌。
请使用至少 32 个随机字符。

是否自动生成安全的 JWT 密钥?
  1. 自动生成
  2. 手动输入
请选择 [1-2]:

访问令牌过期时间（秒）[86400]:
```

### 6. 配置验证

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ✓ 配置验证
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ 配置验证通过
```

当前验证逻辑会额外检查：

- 端口号不能小于 `1024`
- SQLite 父目录不存在时会自动创建
- JWT 密钥长度少于 `32` 字符时会发出警告并询问是否继续

### 7. 保存配置

```text
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💾 保存配置
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ 配置已保存到: ./config.toml
✓ 环境变量已保存到: ./.env
```

### 8. 完成

```text
╔════════════════════════════════════════════════╗
║                                                ║
║     🎉 配置完成！                              ║
║                                                ║
╚════════════════════════════════════════════════╝

下一步操作:
  1. 启动 uHorse:
     ./start.sh

  2. 查看服务状态:
     curl http://127.0.0.1:8765/api/health

> 补充说明：当前 Hub 还提供标准 Prometheus 指标端点 `/metrics`，但不属于该 legacy 向导生成内容。
```

## 命令行选项

```bash
uhorse wizard --help

Options:
  -d, --dir <PATH>    Target directory (default: current)
  -h, --help          Show help message
```

当前向导只支持 `-d/--dir` 与 `-h/--help`。

## 生成的文件

向导会在目标目录写出 `config.toml` 与 `.env`，并覆盖同名现有文件。

当前实现不会从环境变量中读取提示默认值。

### 生成的 `.env`

```bash
# uHorse 环境变量
# 由配置向导生成

UHORSE_SERVER_HOST=127.0.0.1
UHORSE_SERVER_PORT=8765
UHORSE_TELEGRAM_BOT_TOKEN=YOUR_BOT_TOKEN
RUST_LOG=info
```

### 生成的 `config.toml`

典型输出如下：

```toml
# uHorse 配置文件
# 由配置向导生成

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

## 下一步

完成配置后：

```bash
# 使用仓库辅助脚本启动
./start.sh

# 检查健康状态
curl http://127.0.0.1:8765/api/health
```

## 常见问题

### 没有执行权限

```bash
chmod +x ./target/release/uhorse
```

### 重新运行会覆盖已有配置吗？

会。若需保留当前配置，请先备份：

```bash
cp config.toml config.toml.bak
cp .env .env.bak
```

### 系统里没有 `openssl` 怎么办？

自动生成 JWT 密钥依赖 `openssl rand -hex 32`。

如果系统没有 `openssl`：

- 安装 `openssl`；或
- 在向导中选择手动输入 JWT 密钥

### 为什么低端口会校验失败？

当前验证逻辑拒绝使用小于 `1024` 的端口。请改用 `8765` 这类非特权端口。

### 如何配置多个通道？

当前向导一次只会完成 1 个通道的交互配置。若需多通道，请先生成基础配置，再手动补充 `config.toml` 中的其他通道配置段。

## 相关文档

- [配置指南](CONFIG.md) - 完整配置说明
- [API 使用指南](API.md) - API 文档和使用示例
- [通道集成指南](CHANNELS.md) - 各通道集成步骤
- [部署指南](deployments/DEPLOYMENT.md) - 生产环境部署
