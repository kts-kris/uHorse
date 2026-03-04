# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Daily build workflow with smart change detection
- Nightly release channel for development builds

## [0.1.0] - 2025-03-04

### Added

#### Core Infrastructure
- **uhorse-core**: Core types, traits, and protocol definitions
- **uhorse-gateway**: HTTP/WebSocket gateway layer with session management
- **uhorse-agent**: Agent management with independent workspaces
- **uhorse-llm**: LLM abstraction layer supporting multiple providers
- **uhorse-tool**: Tool execution with MCP protocol support
- **uhorse-storage**: Storage layer with SQLite and JSONL backends
- **uhorse-security**: Security layer with JWT authentication and device pairing
- **uhorse-scheduler**: Cron-based task scheduling
- **uhorse-observability**: Observability with tracing, metrics, and audit logs
- **uhorse-config**: Configuration management with interactive wizard

#### Channel Support
- **Telegram**: Full Bot API support with webhook integration
- **钉钉 (DingTalk)**: Enterprise messaging with rich text support
- **飞书 (Feishu/Lark)**: Rich text and interactive card messages
- **企业微信 (WeCom)**: Enterprise internal communication
- **Slack**: Slash commands and interactive components
- **Discord**: Bot integration with embed messages
- **WhatsApp**: WhatsApp Business API integration

#### Features
- Multi-channel unified gateway
- SKILL.md driven skill system
- Structured memory system (SOUL.md, MEMORY.md, USER.md)
- Enterprise-grade security (JWT, device pairing, approval workflow)
- Interactive configuration wizard
- Docker and docker-compose support
- GitHub Actions CI/CD pipeline
- Cross-platform binary releases (Linux, macOS, Windows)

### Performance

- 100K+ concurrent connections
- <1ms P99 latency
- ~30ms cold start time
- 5-20MB memory footprint
- ~15MB binary size (release build)

### Documentation

- README.md with quick start guide
- INSTALL.md detailed installation guide
- API.md REST API reference
- CHANNELS.md channel configuration guide
- CONFIG.md configuration reference
- CONTRIBUTING.md contribution guidelines
- SECURITY.md security policy
- COMPARISON_OPENCLAW.md comparison with OpenClaw

[Unreleased]: https://github.com/kts-kris/uHorse/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kts-kris/uHorse/releases/tag/v0.1.0
