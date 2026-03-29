# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [4.1.2] - 2026-03-29

### Changed

- README / INSTALL / CHANNELS / scripts / release docs are now aligned to `v4.1.2` and match the current Hub-Node, DingTalk, and Node Desktop implementation
- the layered `memory / agent / skill` sharing docs are now aligned to the implemented chain `global / tenant / enterprise / department / role / user / session`
- task context, runtime session, and Web API docs are now aligned to the current `execution_workspace_id`, `collaboration_workspace_id`, `CollaborationWorkspace`, and `/api/v1/sessions*` response shape

## [4.1.1] - 2026-03-27

### Added

- DingTalk natural-language requests can now be planned into a controlled `BrowserCommand`, opening public `http/https` pages and returning extracted page text through the Hub → Node → Hub pipeline
- Hub now applies local safety validation to browser targets and rejects `file://`, localhost, private-network, and other out-of-bound targets
- `uhorse-node-runtime` now uses the formal browser execution path, `uhorse-node-desktop` enables the `browser` feature by default, and browser-capable nodes participate in capability-based routing via `CommandType::Browser`
- GitHub release / nightly workflows now produce mainstream-platform archive artifacts for `uhorse-hub` and `uhorse-node-desktop`
- `channels.dingtalk.notification_bindings` is now part of the documented mainline path for mapping stable `node_id` values to DingTalk `user_id` values so Node Desktop notification mirroring can return to the correct user

### Changed

- The formal Node Desktop delivery boundary is now fixed as a `bin + web` archive together with `package-node-desktop.sh`, `desktop-smoke.sh`, and release artifacts
- README / INSTALL / CHANNELS / scripts / release docs are now aligned to `v4.1.1` and match the current Hub-Node, DingTalk, and Node Desktop implementation
- Daily build and formal release flows now use the `Cargo.toml` version and the `CHANGELOG.md` version section as the release source of truth
- the 4.1 `memory / agent / skill` narrative is now aligned to the layered chain `global / tenant / enterprise / department / role / user / session`, not to a return of the old monolithic Agent platform
- task context and runtime sessions now explicitly distinguish the stable `execution_workspace_id` from the Hub-side logical `collaboration_workspace_id` / `CollaborationWorkspace`
- the runtime API and Web UI expose source-aware metadata through `source_layer` and `source_scope`, so same-name resources from different sources can be distinguished; `/api/v1/sessions*` also return `namespace`, `memory_context_chain`, `visibility_chain`, and `collaboration_workspace`

### Not Included

- `v4.1.1` does not include native `.app/.dmg`, code signing, notarization, installers, or drag-and-drop desktop distribution
- `v4.1.1` does not mean a full return of the old standalone `agent / skill / memory` platform; the docs only describe the layered runtime capabilities that exist in the current Hub-Node mainline
- `v4.1.1` does not restore the legacy monolithic `uhorse` path as a primary deliverable; the primary deliverables remain `uhorse-hub` and `uhorse-node-desktop`

## [4.0.0] - 2026-03-18

### Added

#### Hub-Node Distributed Architecture
- **uhorse-protocol**: Hub-Node communication protocol
  - Message type definitions (HubToNode, NodeToHub)
  - Binary codec (MessageCodec)
  - Command system (Shell, File, Edit, Write, Task, LLM, Info)
  - Task context and priority scheduling
  - Node capabilities and status management

- **uhorse-node**: Local node
  - Task execution engine (TaskExecutor)
  - File system operations (FileOps)
  - Shell command execution
  - Workspace management
  - WebSocket connection to Hub
  - Heartbeat and health reporting

- **uhorse-hub**: Cloud Hub
  - Multi-node management and scheduling
  - Priority task queue
  - Load balancing strategies
  - Node health monitoring
  - Statistics collection

#### Security Features
- **JWT Authentication**: NodeAuthenticator
  - Token issuance and verification
  - Token refresh mechanism
  - Node authentication state management

- **Sensitive Operation Approval**: SensitiveOperationApprover
  - 5 sensitive operation types detection (file_delete, system_command, network_access, credential_access, config_change)
  - Approval workflow (request, approve, reject)
  - Idempotency check

- **Field Encryption**: HubFieldEncryptor
  - AES-GCM encryption
  - JSON field encryption
  - Master key management

- **TLS Configuration**: HubTlsConfig
  - Certificate path configuration
  - Secure transport support

### Testing
- **E2E Tests**: 12 tests covering Hub-Node communication
- **Integration Tests**: 7 tests covering Hub basic functionality
- **Security Tests**: 26 tests covering JWT, encryption, approval
- **Performance Benchmarks**: 8 criterion benchmarks

### Performance
- Task submission throughput optimization
- Concurrent task processing
- Priority scheduling efficiency

## [3.0.0] - 2026-03-10

### Added

#### Enterprise Infrastructure
- **uhorse-discovery**: Service discovery (etcd/consul)
- **uhorse-cache**: Distributed cache (Redis)
- **uhorse-queue**: Message queue (NATS)
- **uhorse-gdpr**: GDPR compliance (data export, erasure, consent)
- **uhorse-governance**: Data governance (classification, retention)
- **uhorse-backup**: Backup and recovery (auto backup, encryption)
- **uhorse-sso**: SSO/OAuth2/OIDC/SAML
- **uhorse-siem**: SIEM integration (Splunk/Datadog)
- **uhorse-webhook**: Webhook enhancement (retry, signature)
- **uhorse-integration**: Third-party integration (Jira/GitHub/Slack)

#### High Availability
- Service registration and discovery
- Load balancing strategies (round-robin, weighted, health-aware)
- Distributed configuration center
- Automatic failover

#### Scalability
- Database sharding
- Read-write separation
- Distributed session cache
- Token blacklist persistence
- Async task queue

#### Compliance
- GDPR/CCPA compliance
- Data classification (4 sensitivity levels)
- Auto backup and recovery
- Audit log persistence

## [2.0.0] - 2026-03-05

### Added

#### Real-time Communication
- **WebSocket Support**: Full bidirectional real-time communication
  - Connection management with heartbeat and reconnection
  - Room-based pub/sub (global, agent, session)
  - Event broadcasting for messages, state changes, task progress
- **SSE (Server-Sent Events)**: Streaming event delivery
  - `/api/v1/events` endpoint for real-time updates
  - `/api/v1/chat/stream` for LLM streaming responses
  - Keep-alive support

#### Frontend Management UI
- **Dashboard**: System overview with metrics and statistics
- **Agents Page**: Agent CRUD, enable/disable, configuration
- **Skills Page**: Skill management with parameter definitions
- **Sessions Page**: Session list, details, message history
- **Channels Page**: Channel status monitoring and configuration
- **Settings Page**: System configuration with tabs
  - General settings (server, logging)
  - LLM settings (model, API key, parameters)
  - Security settings (JWT, rate limiting, CORS)

#### Enterprise Features
- **RBAC (Role-Based Access Control)**:
  - Roles: Admin, Operator, Viewer
  - Resources: Agent, Skill, Session, Channel, System, Tenant
  - Actions: Create, Read, Update, Delete, Execute, Manage
- **Audit Logging**:
  - Operation logging with user/IP tracking
  - Query API with filtering and pagination
  - Export functionality (JSON/CSV)
- **Multi-tenancy**:
  - Tenant isolation with TenantId
  - Resource quotas (agents, skills, messages, storage)
  - Tenant plans: Free, Pro, Enterprise
  - Usage tracking and billing support

#### Multi-modal Support (uhorse-multimodal crate)
- **STT (Speech-to-Text)**: OpenAI Whisper integration
  - Multi-language support
  - Automatic language detection
- **TTS (Text-to-Speech)**: OpenAI TTS integration
  - 6 voice options: alloy, echo, fable, onyx, nova, shimmer
  - Adjustable speed
- **Vision**: Image understanding
  - OpenAI GPT-4V support
  - Anthropic Claude Vision support
  - Base64 and URL image inputs
- **Document Parsing**:
  - PDF text extraction
  - Word (DOCX) parsing
  - Excel (XLSX) reading
  - Markdown/JSON/CSV support

### Changed
- Improved API handler implementations with full CRUD operations
- Enhanced channel implementations with better error handling
- Updated CI configuration with stricter clippy checks

### Dependencies
- Added `async-stream` for streaming support
- Added `futures` for async utilities
- Added `base64` for image encoding
- Added `utoipa` and `utoipa-swagger-ui` for OpenAPI docs

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
- **DingTalk**: Enterprise messaging with rich text support
- **Feishu/Lark**: Rich text and interactive card messages
- **WeCom**: Enterprise internal communication
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

[Unreleased]: https://github.com/kts-kris/uHorse/compare/v4.1.2...HEAD
[4.1.2]: https://github.com/kts-kris/uHorse/compare/v4.1.1...v4.1.2
[4.1.1]: https://github.com/kts-kris/uHorse/compare/v4.0.0...v4.1.1
[4.0.0]: https://github.com/kts-kris/uHorse/compare/v3.0.0...v4.0.0
[3.0.0]: https://github.com/kts-kris/uHorse/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/kts-kris/uHorse/compare/v0.1.0...v2.0.0
[0.1.0]: https://github.com/kts-kris/uHorse/releases/tag/v0.1.0
