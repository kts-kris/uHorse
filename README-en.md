<p align="center">
  <strong>English</strong> | <a href="README.md">简体中文</a>
</p>

<p align="center">
  <img src="assets/logo-wide.png" alt="uHorse Logo" style="background-color: white; padding: 20px; border-radius: 10px;" width="400">
</p>

<h1 align="center">uHorse</h1>

<p align="center">
  <strong>🦄 Enterprise AI Infrastructure Platform</strong>
</p>

<p align="center">
  <em>企业级 AI 基础设施平台</em>
</p>

<p align="center">
  <a href="#-what-is-uhorse">Overview</a> •
  <a href="#-key-highlights">Features</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-documentation">Docs</a> •
  <a href="docs/ENTERPRISE_BEST_PRACTICES.md">🏆 Best Practices</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-3.5.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange" alt="Rust Version">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue" alt="License">
  <img src="https://img.shields.io/badge/status-ready-brightgreen" alt="Status">
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
| 📦 **Modular** | 18+ independent crates, combine as needed, flexible extension |
| 🔧 **MCP Protocol** | Full Model Context Protocol support, compatible with mainstream LLM tool ecosystem |

---

## 🆚 Comparison with OpenClaw

OpenClaw is an excellent personal AI assistant framework, while uHorse focuses on **enterprise multi-channel scenarios**:

| Dimension | OpenClaw | uHorse | Recommendation |
|-----------|----------|--------|----------------|
| **Positioning** | Personal AI Assistant | Enterprise AI Gateway | Personal use → OpenClaw, Enterprise → uHorse |
| **Tech Stack** | TypeScript (220K+ lines) | Rust (15K+ lines) | Performance → Rust |
| **Architecture** | 3-Layer (Gateway-Skills-Memory) | 4-Layer (Gateway-Agent-Skills-Memory) | Multi-Agent → uHorse |
| **Channels** | Community plugin driven | Built-in 7+ enterprise channels | Multi-channel → uHorse |
| **Workspace** | Single shared | Independent Agent isolation | Multi-tenant → uHorse |
| **Enterprise Features** | Basic | Auth/AuthZ/Audit/Monitoring/Compliance | Production → uHorse |
| **Performance** | ~10K concurrent | ~100K+ concurrent | High concurrency → uHorse |
| **Memory Footprint** | 50-200MB | 5-20MB | Edge devices → uHorse |
| **High Availability** | Manual setup | Built-in cluster + failover | Enterprise → uHorse |
| **Data Governance** | None | Classification/Retention/Backup | Compliance → uHorse |
| **SSO Integration** | Community plugins | OAuth2/OIDC/SAML built-in | Enterprise SSO → uHorse |

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
├─ Complete Audit / Security Required ───────→ uHorse ✅
├─ GDPR/SOC2 Compliance ─────────────────────→ uHorse ✅
└─ SSO/SIEM Integration ─────────────────────→ uHorse ✅
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
│  • SOUL.md (Constitution)  • MEMORY.md (Long-term)  • USER.md       │
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
├── uhorse-discovery/    # Service discovery (etcd/consul) + failover
├── uhorse-cache/        # Distributed cache (Redis)
├── uhorse-queue/        # Message queue (NATS)
├── uhorse-gdpr/         # GDPR compliance
├── uhorse-governance/   # Data governance (classification/retention)
├── uhorse-backup/       # Backup & recovery
├── uhorse-sso/          # SSO/OAuth2/OIDC/SAML
├── uhorse-siem/         # SIEM integration (Splunk/Datadog)
├── uhorse-webhook/      # Webhook enhancement
├── uhorse-integration/  # Third-party integration (Jira/GitHub/Slack)
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

### 🏆 Enterprise Guides

| Document | Description |
|----------|-------------|
| **[Enterprise Best Practices Guide](docs/ENTERPRISE_BEST_PRACTICES-en.md)** | ⭐ **Highly Recommended** - 5 typical scenarios, architecture design, deployment & operations, security compliance, cost optimization |

### Basic Documentation

| Document | Description |
|----------|-------------|
| [Installation Guide](INSTALL.md) | Detailed installation steps |
| [Configuration Wizard](WIZARD.md) | Interactive configuration guide |
| [API Reference](API.md) | REST API reference |
| [Channel Integration](CHANNELS.md) | Channel configuration guides |
| [Skill Development](SKILLS.md) | Custom skill development |
| [Deployment Guide](deployments/DEPLOYMENT.md) | Production deployment |

### Architecture & Roadmap

| Document | Description |
|----------|-------------|
| [v3.0 Architecture Design](docs/architecture/v3.0-architecture.md) | Enterprise architecture design |
| [v3.0 Roadmap](docs/roadmap/v3.0-roadmap.md) | Complete development roadmap |
| [Release Notes](RELEASE_NOTES.md) | Version changelog |

---

## 🛣️ Roadmap

| Phase | Name | Duration | Status | Completion Date |
|-------|------|----------|--------|-----------------|
| **Phase 1** | High Availability Infrastructure | 4 weeks | ✅ Complete | 2025-03-01 |
| **Phase 2** | Scalability Architecture | 5 weeks | ✅ Complete | 2025-03-08 |
| **Phase 3** | Security & Compliance | 4 weeks | ✅ Complete | 2025-03-12 |
| **Phase 4** | Data Governance | 3 weeks | ✅ Complete | 2025-03-15 |
| **Phase 5** | API Standards | 3 weeks | ✅ Complete | 2025-03-18 |
| **Phase 6** | Enterprise Integration | 4 weeks | ✅ Complete | 2025-03-13 |

### v3.0 ✅ Released - Enterprise AI Infrastructure Platform

> Upgraded from "Enterprise Multi-Channel AI Gateway" to "Enterprise AI Infrastructure Platform"

**Core Goals Achieved**:

| Dimension | 2.0 Baseline | 3.0 Target | Achieved |
|-----------|--------------|------------|----------|
| **High Availability** | 40% | 95% | ✅ 95% |
| **Scalability** | 40% | 95% | ✅ 95% |
| **Security Compliance** | 50% | 100% | ✅ 100% |
| **Data Governance** | 40% | 100% | ✅ 100% |
| **API Standards** | 60% | 100% | ✅ 100% |
| **Enterprise Integration** | 30% | 100% | ✅ 100% |

**Phase 1 Completed** ✅:
- [x] etcd service discovery
- [x] Consul backup backend
- [x] 4 load balancing strategies (round-robin/weighted/health-aware/least-connection)
- [x] Distributed configuration center
- [x] Hot configuration reload
- [x] Configuration version management

**Phase 2 Completed** ✅:
- [x] Database sharding (by tenant_id)
- [x] Read-write separation (master-slave replication)
- [x] Redis distributed cache (session cache/token blacklist)
- [x] NATS message queue (task queue/dead letter queue)
- [x] Cache policies (LRU/LFU/TTL)

**Phase 3 Completed** ✅:
- [x] TLS 1.3 transport encryption
- [x] Let's Encrypt certificate management
- [x] Database encryption (SQLCipher)
- [x] Field-level encryption
- [x] GDPR compliance (data export/deletion/consent management)
- [x] Audit log persistence + tamper-proof signing

**Phase 4 Completed** ✅:
- [x] Data classification framework (4 sensitivity levels: Public/Internal/Confidential/Restricted)
- [x] Data retention policy management
- [x] Data archiving mechanism (cold data archive)
- [x] Automatic backup scheduling (full/incremental)
- [x] AES-256-GCM backup encryption
- [x] Point-in-time recovery (PITR)
- [x] Cross-region replication (disaster recovery)
- [x] Automatic failover (auto/manual/priority strategies)

**Phase 5 Completed** ✅:
- [x] OpenAPI 3.0 specification generation (utoipa integration)
- [x] Swagger UI + ReDoc documentation UI
- [x] Client code generators (TypeScript/Go/Python/Rust)
- [x] API version management (URL version + deprecation notice)
- [x] Compatibility checker (breaking change detection)
- [x] Rate Limiting (global/user/endpoint/distributed)

**Phase 6 Completed** ✅:
- [x] OAuth2 authorization server (authorization code/client credentials/refresh token)
- [x] OIDC client (identity discovery/user info/token validation)
- [x] SAML 2.0 client (enterprise SSO integration)
- [x] Multi-IdP integration (Okta/Auth0/Azure AD/Google Workspace)
- [x] SIEM integration (Splunk HEC/Datadog Logs API)
- [x] Audit log export (JSON/CEF/Syslog/CSV)
- [x] Security alert management (rule engine/threshold detection)
- [x] Webhook enhancement (retry/signature/template/history)
- [x] Third-party integration (Jira/GitHub/Slack)

📄 **Full Documentation**: [v3.0 Roadmap](docs/roadmap/v3.0-roadmap.md) | [Architecture Design](docs/architecture/v3.0-architecture.md)

### v3.5 🚧 In Progress - Developer Experience Enhancement

> Focus on developer experience and operational efficiency improvements

**Core Objectives**:

| Dimension | 3.0 Baseline | 3.5 Target | Improvement |
|-----------|--------------|------------|-------------|
| **CLI Experience** | Basic commands | TUI interactive | +80% |
| **Error Messages** | Simple text | Structured + suggestions | +90% |
| **Debugging** | Log viewing | Real-time panel | +95% |
| **Quick Start** | Build required | Docker one-click | +100% |
| **SDK Support** | None | Python/TypeScript | From 0 to 1 |

**Phase 1 Completed** ✅:
- [x] CLI TUI interactive enhancement (colored/indicatif/dialoguer/console)
- [x] Error message optimization (error codes + cause analysis + solutions + doc links)
- [x] Playground Docker image (30-second quick experience)
- [x] Preset scenario templates (customer-service/HR/IT-support/sales/general - 5 templates)

**Phase 2 Completed** ✅:
- [x] Web skill editor (online editing + JSON Schema validation + skill template library)
- [x] Debug panel (conversation flow + tool calls + performance metrics + WebSocket real-time updates)
- [x] Enhanced doctor command (auto-fix + dependency check + config validation)
- [x] SDK development (Python SDK + TypeScript SDK)

**Phase 3 Planned**:
- [ ] Interactive tutorial system
- [ ] Example library
- [ ] Dashboard

---

## 🤝 Contributing

Contributions are welcome! Please check [CONTRIBUTING.md](CONTRIBUTING.md).

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
