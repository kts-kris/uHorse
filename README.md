<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>🦄 企业级多渠道 AI 网关 + 智能体框架</strong>
</p>

<p align="center">
  <a href="#特性">特性</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#架构">架构</a> •
  <a href="#与-openclaw-对比">对比</a> •
  <a href="#文档">文档</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-2.0.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-production%20ready-green" alt="Status">
</p>

---

## 🌟 uHorse 是什么？

uHorse 是一个用 **Rust** 编写的企业级多渠道 AI 网关和智能体框架。它将大语言模型（LLM）的力量连接到 7+ 主流通信平台，让 AI 助手能够无缝地在 Telegram、钉钉、飞书、企业微信、Slack、Discord、WhatsApp 等平台上为用户服务。

```bash
# 一句话概括
uHorse = 多渠道网关 + 智能体编排 + 技能系统 + 记忆管理
```

### ✨ 核心亮点

| 特性 | 说明 |
|------|------|
| 🚀 **高性能** | Rust + Tokio 异步运行时，单机支持 100K+ 并发连接 |
| 🔌 **7+ 通道** | Telegram、钉钉⭐、飞书、企业微信、Slack、Discord、WhatsApp |
| 🤖 **多智能体** | 独立 Agent 工作空间，支持多 Agent 协作 |
| 🛡️ **企业级** | JWT 认证、设备配对、审批流程、完整审计日志 |
| 📦 **模块化** | 10+ 独立 crate，按需组合，灵活扩展 |
| 🔧 **MCP 协议** | 完整支持 Model Context Protocol，兼容主流 LLM 工具生态 |

---

## 🆚 与 OpenClaw 对比

OpenClaw 是优秀的个人 AI 助手框架，uHorse 则专注于**企业级多渠道场景**：

| 维度 | OpenClaw | uHorse | 选择建议 |
|------|----------|--------|----------|
| **定位** | 个人 AI 助手 | 企业 AI 网关 | 个人用 OpenClaw，企业用 uHorse |
| **技术栈** | TypeScript (220K+ 行) | Rust (10K+ 行) | 追求性能用 Rust |
| **架构** | 3层 (Gateway-Skills-Memory) | 4层 (Gateway-Agent-Skills-Memory) | 多 Agent 场景用 uHorse |
| **通道** | 社区插件驱动 | 内置 7+ 企业通道 | 需要多渠道用 uHorse |
| **工作空间** | 单一共享 | 独立 Agent 隔离 | 多租户场景用 uHorse |
| **企业功能** | 基础 | 认证/授权/审计/监控 | 生产环境用 uHorse |
| **性能** | ~10K 并发 | ~100K+ 并发 | 高并发场景用 uHorse |
| **内存占用** | 50-200MB | 5-20MB | 边缘设备用 uHorse |

### 决策树

```
你的需求是什么？
├─ 个人 AI 助手 ────────────────────→ OpenClaw ✅
├─ 快速原型开发 (TypeScript) ───────→ OpenClaw ✅
├─ 利用社区插件生态 ────────────────→ OpenClaw ✅
│
├─ 企业生产部署 ────────────────────→ uHorse ✅
├─ 多渠道统一接入 ──────────────────→ uHorse ✅
├─ 多智能体协作 ────────────────────→ uHorse ✅
├─ 高并发/低延迟 ───────────────────→ uHorse ✅
├─ 边缘计算/资源受限 ───────────────→ uHorse ✅
└─ 需要完整审计/安全 ───────────────→ uHorse ✅
```

---

## 🏗️ 架构

uHorse 采用**四层架构**，相比传统的三层架构增加了独立的智能体层：

```
┌─────────────────────────────────────────────────────────────────────┐
│                        🌐 Gateway (控制平面)                         │
│  • 会话管理  • 消息路由  • Bindings 规则引擎  • 事件驱动架构          │
│  • 通道: Telegram ⭐ | 钉钉 ⭐ | 飞书 | 企业微信 | Slack | Discord    │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🤖 Agent (智能体层)                           │
│  • LLM 调用编排  • 工具使用决策  • 意图识别  • 多 Agent 协作         │
│  • 独立工作空间: ~/.uhorse/workspace-{agent_name}/                  │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🔧 Skills (技能系统)                          │
│  • SKILL.md 驱动  • Rust/WASM 执行  • JSON Schema 验证  • 权限控制  │
│  • MCP Tools 集成  • 内置: calculator, time, text_search           │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🧠 Memory (记忆系统)                          │
│  • SOUL.md (宪法/行为准则)  • MEMORY.md (长期记忆)  • USER.md       │
│  • 文件系统 + SQLite 持久化  • SessionState 结构化管理              │
└─────────────────────────────────────────────────────────────────────┘
```

### 模块结构

```
uhorse/
├── uhorse-core/         # 核心类型、Trait、协议定义
├── uhorse-gateway/      # HTTP/WebSocket 网关层
├── uhorse-channel/      # 通道适配器 (7+ 通道)
├── uhorse-agent/        # 智能体管理、会话管理
├── uhorse-llm/          # LLM 抽象层 (OpenAI, Anthropic, ...)
├── uhorse-tool/         # 工具执行、MCP 协议
├── uhorse-storage/      # 存储层 (SQLite, JSONL)
├── uhorse-security/     # 安全层 (JWT, 设备配对, 审批)
├── uhorse-scheduler/    # Cron 调度器
├── uhorse-observability/# 可观测性 (tracing, metrics, audit)
├── uhorse-config/       # 配置管理、交互式向导
└── uhorse-bin/          # 二进制程序入口
```

---

## 🚀 快速开始

### 方式一：一键安装 ⭐ 推荐

```bash
# 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 一键安装（自动检查依赖、编译、配置）
./install.sh
```

### 方式二：交互式配置向导

```bash
# 编译
cargo build --release

# 启动配置向导
./target/release/uhorse wizard
```

向导将引导你配置：
- 📡 服务器地址和端口
- 💾 数据库（SQLite 或 PostgreSQL）
- 📱 通道凭证（选择你需要的通道）
- 🤖 LLM 配置（OpenAI、Anthropic、Gemini...）
- 🔒 安全设置（JWT 密钥、Token 过期时间）

### 方式三：Docker

```bash
docker-compose up -d
```

### 验证安装

```bash
# 健康检查
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# 查看指标
curl http://localhost:8080/metrics
```

---

## 📱 支持的通道

| 通道 | 状态 | 标签 | 说明 |
|------|------|------|------|
| **Telegram** | ✅ 稳定 | ⭐ 默认预装 | 最成熟的通道，完整 Bot API 支持 |
| **钉钉** | ✅ 稳定 | ⭐ 默认预装 | 企业级，支持富文本、卡片消息 |
| **飞书** | ✅ 稳定 | 新增 | 支持富文本、交互式卡片 |
| **企业微信** | ✅ 稳定 | 新增 | 企业内部沟通首选 |
| **Slack** | ✅ 稳定 | - | 完整 Slash Commands 支持 |
| **Discord** | ✅ 稳定 | - | 游戏社区、Embed 消息 |
| **WhatsApp** | ✅ 稳定 | - | WhatsApp Business API |

### 配置示例

```toml
# config.toml

[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = ["telegram", "dingtalk"]  # 启用的通道

[channels.telegram]
bot_token = "your_bot_token"
webhook_secret = "optional_secret"

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789

[database]
path = "./data/uhorse.db"

[llm]
enabled = true
provider = "openai"
api_key = "sk-..."
model = "gpt-4"
```

---

## 🔧 核心功能

### 1. 多渠道统一网关

```rust
// 统一的通道接口
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> ChannelType;
    async fn send_message(&self, user_id: &str, message: &MessageContent) -> Result<(), ChannelError>;
    async fn verify_webhook(&self, payload: &[u8], signature: Option<&str>) -> Result<bool, ChannelError>;
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn is_running(&self) -> bool;
}
```

### 2. SKILL.md 驱动的技能系统

```markdown
# 天气查询技能

## Description
查询全球任意城市的实时天气信息

## Version
1.0.0

## Tags
weather,api,utility

## Tools
{
  "name": "get_weather",
  "description": "获取指定城市的天气",
  "inputSchema": {
    "type": "object",
    "properties": {
      "city": {"type": "string", "description": "城市名称"},
      "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
    },
    "required": ["city"]
  }
}
```

### 3. 结构化记忆系统

```
~/.uhorse/
├── workspace-main/       # 主 Agent 工作空间
│   ├── SOUL.md          # 宪法 - 定义行为准则
│   ├── MEMORY.md        # 长期记忆索引
│   ├── USER.md          # 用户偏好
│   └── sessions/        # 会话状态
├── workspace-coder/     # Coder Agent（独立个性）
│   └── SOUL.md          # 专注于代码的"灵魂"
└── workspace-writer/    # Writer Agent（独立个性）
    └── SOUL.md          # 专注于写作的"灵魂"
```

### 4. 企业级安全

- **JWT 认证**: 安全的 Token 验证
- **设备配对**: 新设备需要审批
- **审批流程**: 敏感操作需人工确认
- **审计日志**: 完整操作记录
- **幂等控制**: 防止重复操作

---

## 📊 性能

| 指标 | 数值 | 说明 |
|------|------|------|
| **并发连接** | 100K+ | Tokio 异步运行时 |
| **请求延迟** | <1ms | P99 延迟 |
| **启动时间** | ~30ms | 无 JIT 预热 |
| **内存占用** | 5-20MB | 无 GC 开销 |
| **二进制大小** | ~15MB | Release 编译 |

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [安装指南](INSTALL.md) | 详细安装步骤 |
| [配置向导](WIZARD.md) | 交互式配置说明 |
| [API 文档](API.md) | REST API 参考 |
| [通道集成](CHANNELS.md) | 各通道配置指南 |
| [技能开发](SKILLS.md) | 自定义技能开发 |
| [部署指南](deployments/DEPLOYMENT.md) | 生产环境部署 |

---

## 🛣️ 路线图

### v1.0 ✅ 生产就绪
- [x] 核心基础设施
- [x] 7+ 通道集成
- [x] 工具与插件系统
- [x] 调度与安全增强
- [x] 可观测性完善

### v2.0 ✅ 已发布
- [x] **API 完善**: 完整的 REST API (Agents/Skills/Sessions/Files/Channels)
- [x] **通道实现**: Telegram/钉钉/飞书/企业微信/Slack/Discord/WhatsApp
- [x] **实时通信**: WebSocket 连接管理 + SSE 流式响应
- [x] **前端完善**: React 管理界面 (Agent/Skill/Session/Channel 管理)
- [x] **企业级特性**: RBAC 权限系统 + 审计日志 + 多租户架构
- [x] **多模态支持**: STT/TTS 语音处理 + Vision 图像理解 + 文件解析

### v2.1 📋 计划中
- [ ] 联邦学习
- [ ] 边缘部署优化
- [ ] 技能市场
- [ ] 更多 LLM 提供商

---

## 🤝 贡献

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md)。

### 开发环境

```bash
# 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 安装开发依赖
cargo install cargo-watch cargo-nextest

# 运行测试
cargo nextest run

# 热重载开发
cargo watch -x run
```

---

## 📄 许可证

双许可：[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE)

---

## 🙏 致谢

- 感谢 [OpenClaw](https://github.com/openclaw/openclaw) 团队在 AI 助手领域的探索，为社区提供了宝贵的参考
- 感谢所有贡献者

---

<p align="center">
  <strong>uHorse - 让 AI 无处不在</strong>
</p>
