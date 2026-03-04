<p align="center">
  <img src="docs/assets/uhorse-logo.png" alt="uHorse Logo" width="200">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>🦄 Enterprise Multi-Channel AI Gateway + Agent Framework</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#comparison-with-openclaw">Comparison</a> •
  <a href="#documentation">Docs</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-production%20ready-green" alt="Status">
</p>

---

## 🌟 What is uHorse?

uHorse is an enterprise-grade multi-channel AI gateway and agent framework written in **Rust**. It connects the power of Large Language Models (LLMs) to 7+ major communication platforms, enabling AI assistants to seamlessly serve users on Telegram, DingTalk, Feishu, WeCom, Slack, Discord, WhatsApp, and more.

```bash
# One-liner summary
uHorse = Multi-Channel Gateway + Agent Orchestration + Skill System + Memory Management
```

### ✨ Key Highlights

| Feature | Description |
|---------|-------------|
| 🚀 **High Performance** | Rust + Tokio async runtime, 100K+ concurrent connections on single machine |
| 🔌 **7+ Channels** | Telegram, DingTalk⭐, Feishu, WeCom, Slack, Discord, WhatsApp |
| 🤖 **Multi-Agent** | Independent Agent workspaces, multi-agent collaboration support |
| 🛡️ **Enterprise-Grade** | JWT authentication, device pairing, approval workflows, complete audit logs |
| 📦 **Modular** | 10+ independent crates, combine as needed, flexible extension |
| 🔧 **MCP Protocol** | Full Model Context Protocol support, compatible with mainstream LLM tool ecosystem |

---

## 🆚 Comparison with OpenClaw

OpenClaw is an excellent personal AI assistant framework, while uHorse focuses on **enterprise multi-channel scenarios**:

| Dimension | OpenClaw | uHorse | Recommendation |
|-----------|----------|--------|----------------|
| **Positioning** | Personal AI Assistant | Enterprise AI Gateway | Personal use → OpenClaw, Enterprise → uHorse |
| **Tech Stack** | TypeScript (220K+ lines) | Rust (10K+ lines) | Performance → Rust |
| **Architecture** | 3-Layer (Gateway-Skills-Memory) | 4-Layer (Gateway-Agent-Skills-Memory) | Multi-Agent → uHorse |
| **Channels** | Community plugin driven | Built-in 7+ enterprise channels | Multi-channel → uHorse |
| **Workspace** | Single shared | Independent Agent isolation | Multi-tenant → uHorse |
| **Enterprise Features** | Basic | Auth/AuthZ/Audit/Monitoring | Production → uHorse |
| **Performance** | ~10K concurrent | ~100K+ concurrent | High concurrency → uHorse |
| **Memory Footprint** | 50-200MB | 5-20MB | Edge devices → uHorse |

### Decision Tree

```
What are your needs?
├─ Personal AI Assistant ────────────────────→ OpenClaw ✅
├─ Quick Prototyping (TypeScript) ────────────→ OpenClaw ✅
├─ Leverage Community Plugin Ecosystem ──────→ OpenClaw ✅
│
├─ Enterprise Production Deployment ─────────→ uHorse ✅
├─ Multi-Channel Unified Access ─────────────→ uHorse ✅
├─ Multi-Agent Collaboration ─────────────────→ uHorse ✅
├─ High Concurrency / Low Latency ───────────→ uHorse ✅
├─ Edge Computing / Resource Constrained ────→ uHorse ✅
└─ Complete Audit / Security Required ───────→ uHorse ✅
```

---

## 🏗️ Architecture

uHorse adopts a **four-layer architecture**, adding an independent agent layer compared to traditional three-layer architecture:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        🌐 Gateway (Control Plane)                    │
│  • Session Management  • Message Routing  • Bindings Rule Engine    │
│  • Event-Driven Architecture                                          │
│  • Channels: Telegram ⭐ | DingTalk ⭐ | Feishu | WeCom | Slack      │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🤖 Agent (Intelligence Layer)                 │
│  • LLM Orchestration  • Tool Usage Decision  • Intent Recognition   │
│  • Multi-Agent Collaboration                                         │
│  • Independent Workspace: ~/.uhorse/workspace-{agent_name}/         │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🔧 Skills (Skill System)                      │
│  • SKILL.md Driven  • Rust/WASM Execution  • JSON Schema Validation │
│  • Permission Control  • MCP Tools Integration                       │
│  • Built-in: calculator, time, text_search                          │
└─────────────────────────────────────────────────────────────────────┘
                                 ↓
┌─────────────────────────────────────────────────────────────────────┐
│                        🧠 Memory (Memory System)                     │
│  • SOUL.md (Constitution)  • MEMORY.md (Long-term)  • USER.md      │
│  • File System + SQLite Persistence  • SessionState Management      │
└─────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
uhorse/
├── uhorse-core/         # Core types, traits, protocol definitions
├── uhorse-gateway/      # HTTP/WebSocket gateway layer
├── uhorse-channel/      # Channel adapters (7+ channels)
├── uhorse-agent/        # Agent management, session management
├── uhorse-llm/          # LLM abstraction layer (OpenAI, Anthropic, ...)
├── uhorse-tool/         # Tool execution, MCP protocol
├── uhorse-storage/      # Storage layer (SQLite, JSONL)
├── uhorse-security/     # Security layer (JWT, device pairing, approval)
├── uhorse-scheduler/    # Cron scheduler
├── uhorse-observability/# Observability (tracing, metrics, audit)
├── uhorse-config/       # Configuration management, interactive wizard
└── uhorse-bin/          # Binary entry point
```

---

## 🚀 Quick Start

### Option 1: One-Click Install ⭐ Recommended

```bash
# Clone repository
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# One-click install (auto-check dependencies, compile, configure)
./install.sh
```

### Option 2: Interactive Configuration Wizard

```bash
# Build
cargo build --release

# Start configuration wizard
./target/release/uhorse wizard
```

The wizard will guide you through:
- 📡 Server address and port
- 💾 Database (SQLite or PostgreSQL)
- 📱 Channel credentials (select channels you need)
- 🤖 LLM configuration (OpenAI, Anthropic, Gemini...)
- 🔒 Security settings (JWT secret, token expiration)

### Option 3: Docker

```bash
docker-compose up -d
```

### Verify Installation

```bash
# Health check
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# View metrics
curl http://localhost:8080/metrics
```

---

## 📱 Supported Channels

| Channel | Status | Tag | Description |
|---------|--------|-----|-------------|
| **Telegram** | ✅ Stable | ⭐ Default | Most mature channel, full Bot API support |
| **DingTalk** | ✅ Stable | ⭐ Default | Enterprise-grade, rich text and card messages |
| **Feishu** | ✅ Stable | New | Rich text and interactive card support |
| **WeCom** | ✅ Stable | New | Enterprise internal communication |
| **Slack** | ✅ Stable | - | Full Slash Commands support |
| **Discord** | ✅ Stable | - | Gaming community, embed messages |
| **WhatsApp** | ✅ Stable | - | WhatsApp Business API |

### Configuration Example

```toml
# config.toml

[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = ["telegram", "dingtalk"]  # Enabled channels

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

## 🔧 Core Features

### 1. Multi-Channel Unified Gateway

```rust
// Unified channel interface
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> ChannelType;
    async fn send_message(&self, user_id: &str, message: &MessageContent) -> Result<(), ChannelError>;
    async fn verify_webhook(&self, payload: &[u8], signature: Option<&str>) -> Result<bool, ChannelError>;
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn is_running(&self) -> bool;
}
```

### 2. SKILL.md Driven Skill System

```markdown
# Weather Query Skill

## Description
Query real-time weather information for any city worldwide

## Version
1.0.0

## Tags
weather, api, utility

## Tools
{
  "name": "get_weather",
  "description": "Get weather for specified city",
  "inputSchema": {
    "type": "object",
    "properties": {
      "city": {"type": "string", "description": "City name"},
      "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
    },
    "required": ["city"]
  }
}
```

### 3. Structured Memory System

```
~/.uhorse/
├── workspace-main/       # Main Agent workspace
│   ├── SOUL.md          # Constitution - defines behavior guidelines
│   ├── MEMORY.md        # Long-term memory index
│   ├── USER.md          # User preferences
│   └── sessions/        # Session states
├── workspace-coder/     # Coder Agent (independent personality)
│   └── SOUL.md          # Code-focused "soul"
└── workspace-writer/    # Writer Agent (independent personality)
    └── SOUL.md          # Writing-focused "soul"
```

### 4. Enterprise-Grade Security

- **JWT Authentication**: Secure token verification
- **Device Pairing**: New device requires approval
- **Approval Workflow**: Sensitive operations need human confirmation
- **Audit Logs**: Complete operation records
- **Idempotency Control**: Prevent duplicate operations

---

## 📊 Performance

| Metric | Value | Description |
|--------|-------|-------------|
| **Concurrent Connections** | 100K+ | Tokio async runtime |
| **Request Latency** | <1ms | P99 latency |
| **Startup Time** | ~30ms | No JIT warmup |
| **Memory Footprint** | 5-20MB | No GC overhead |
| **Binary Size** | ~15MB | Release build |

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Installation Guide](INSTALL-en.md) | Detailed installation steps |
| [Configuration Wizard](WIZARD.md) | Interactive configuration guide |
| [API Reference](API.md) | REST API reference |
| [Channel Integration](CHANNELS.md) | Channel configuration guides |
| [Skill Development](SKILLS-en.md) | Custom skill development |
| [Deployment Guide](deployments/DEPLOYMENT.md) | Production deployment |

---

## 🛣️ Roadmap

### v1.0 ✅ Production Ready
- [x] Core infrastructure
- [x] 7+ channel integration
- [x] Tool and plugin system
- [x] Scheduling and security enhancements
- [x] Observability completion

### v1.1 🚧 In Progress
- [ ] Web management interface
- [ ] More LLM providers
- [ ] Skill marketplace

### v2.0 📋 Planned
- [ ] Multi-tenant support
- [ ] Federated learning
- [ ] Edge deployment optimization

---

## 🤝 Contributing

Contributions are welcome! Please check [CONTRIBUTING-en.md](CONTRIBUTING-en.md).

### Development Environment

```bash
# Clone repository
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# Install development dependencies
cargo install cargo-watch cargo-nextest

# Run tests
cargo nextest run

# Hot reload development
cargo watch -x run
```

---

## 📄 License

Dual licensed: [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE)

---

## 🙏 Acknowledgments

- Thanks to the [OpenClaw](https://github.com/openclaw/openclaw) team for their exploration in the AI assistant field, providing valuable reference for the community
- Thanks to all contributors

---

<p align="center">
  <strong>uHorse - Making AI Ubiquitous</strong>
</p>
