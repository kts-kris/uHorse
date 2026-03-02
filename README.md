# OpenClaw AI Gateway (Rust)

多渠道 AI 网关框架 - Rust 实现。

## 项目状态

✅ **v1.0.0 生产就绪** - 所有 7 个阶段已完成

- ✅ Phase 1: 核心基础设施
- ✅ Phase 2: 通道集成 (Telegram, Slack, Discord, WhatsApp)
- ✅ Phase 3: 工具与插件系统
- ✅ Phase 4: 调度与安全增强
- ✅ Phase 5: 可观测性完善
- ✅ Phase 6: 生产环境准备
- ✅ Phase 7: 生产环境部署

[查看详细进度 →](PROGRESS.md)

## 架构概览

```
openclaw-rs/
├── openclaw-core/        # 核心类型和 trait
├── openclaw-gateway/      # 网关层 (HTTP/WebSocket)
├── openclaw-storage/      # 存储层 (SQLite/JSONL)
├── openclaw-session/      # 会话层
├── openclaw-channel/      # 通道适配器
├── openclaw-tool/         # 工具执行层
├── openclaw-security/     # 安全层 (认证/授权)
├── openclaw-scheduler/    # 调度器
├── openclaw-observability/# 可观测性
└── openclaw-bin/          # 二进制程序
```

## 开发路线图

### Phase 1: 核心基础设施 ✅ (进行中)
- [x] Workspace 结构
- [x] 核心类型定义
- [x] 协议定义
- [x] SQLite 存储层
- [x] WebSocket 处理器
- [ ] 会话管理器完善
- [ ] 工具执行引擎

### Phase 2: 通道集成 (待开始)
- [ ] Telegram Bot
- [ ] Slack 集成
- [ ] Discord Bot
- [ ] WhatsApp Business API

### Phase 3: 工具与插件
- [ ] 工具注册表
- [ ] 参数验证
- [ ] 权限检查
- [ ] 进程插件

### Phase 4: 调度与安全
- [ ] Cron 调度器
- [ ] JWT 认证
- [ ] 设备配对
- [ ] 审批流程

### Phase 5: 可观测性
- [ ] Tracing 集成
- [ ] Metrics 导出
- [ ] 审计日志

## 快速开始

### 方式一：本地开发 (推荐)

```bash
# 1. 克隆仓库
git clone https://github.com/openclaw/openclaw-rs
cd openclaw-rs

# 2. 复制环境配置
cp .env.example .env

# 3. 启动依赖服务 (PostgreSQL + Redis)
docker-compose up -d postgres redis

# 4. 运行数据库迁移
cargo run -- migrate

# 5. 启动应用
./scripts/start.sh
# 或开发模式 (热重载)
./scripts/dev.sh
```

**详细指南**: [本地开发启动指南](LOCAL_SETUP.md)

### 方式二：Docker Compose

```bash
# 启动完整环境
docker-compose up -d

# 查看状态
docker-compose ps

# 查看日志
docker-compose logs -f openclaw
```

**详细指南**: [部署文档](deployments/DEPLOYMENT.md)

## 配置

创建 `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
path = "./data/openclaw.db"

[security]
jwt_secret = "your-secret-key"
token_expiry = 86400
```

## 技术栈

- **Runtime**: Tokio
- **Web**: Axum
- **Database**: SQLite (sqlx)
- **Serialization**: serde
- **Tracing**: tracing + opentelemetry
- **Authentication**: JWT (jsonwebtoken)

## 贡献

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT OR Apache-2.0
