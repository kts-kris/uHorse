# uHorse AI Gateway (Rust)

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
uhorse-rs/
├── uhorse-core/        # 核心类型和 trait
├── uhorse-gateway/      # 网关层 (HTTP/WebSocket)
├── uhorse-storage/      # 存储层 (SQLite/JSONL)
├── uhorse-session/      # 会话层
├── uhorse-channel/      # 通道适配器
├── uhorse-tool/         # 工具执行层
├── uhorse-security/     # 安全层 (认证/授权)
├── uhorse-scheduler/    # 调度器
├── uhorse-observability/# 可观测性
└── uhorse-bin/          # 二进制程序
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

### 方式一：一键安装脚本（最简单）⭐

```bash
# 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 运行一键安装脚本
./install.sh
```

**install.sh 脚本会自动完成：**
- ✅ 检查依赖（Rust、Cargo）
- ✅ 编译项目（Release 模式）
- ✅ 创建必要目录（data/、logs/）
- ✅ 启动配置向导
- ✅ 生成配置文件（config.toml、.env）
- ✅ 可选：立即启动服务

### 方式二：快速设置（默认配置）

```bash
# 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 运行快速设置脚本
./quick-setup.sh
```

**quick-setup.sh 使用场景：**
- 快速本地测试
- 使用默认配置
- 无需通道配置

### 方式三：交互式配置向导（推荐生产环境）

```bash
# 1. 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

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
docker-compose logs -f uhorse
```

**详细指南**: [部署文档](deployments/DEPLOYMENT.md)

## 配置

### 方式一：配置文件

创建 `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = []

[database]
path = "./data/uhorse.db"

[security]
jwt_secret = "your-secret-key"
token_expiry = 86400
```

### 方式二：环境变量

创建 `.env` 文件:

```bash
cp .env.example .env
# 编辑 .env 文件配置您的环境
```

## 📚 文档

| 文档 | 说明 |
|------|------|
| [安装指南](INSTALL.md) | 安装和设置说明 |
| [配置向导](WIZARD.md) | 交互式配置向导 |
| [配置指南](CONFIG.md) | 完整配置说明 |
| [API 使用指南](API.md) | API 文档和使用示例 |
| [通道集成指南](CHANNELS.md) | 各通道集成步骤 |
| [本地开发](LOCAL_SETUP.md) | 本地开发环境 |
| [测试指南](TESTING.md) | 测试说明 |
| [部署指南](deployments/DEPLOYMENT.md) | 生产环境部署 |

## 🚀 快速命令

### 安装和配置

```bash
# 一键安装（推荐）
./install.sh

# 快速设置（默认配置）
./quick-setup.sh

# 仅运行配置向导
./target/release/uhorse wizard
```

### 服务管理

```bash
# 启动服务器
./start.sh                    # 后台启动
./run.sh                      # 前台启动（开发模式）
./target/release/uhorse run   # 直接运行

# 停止服务器
./stop.sh

# 重启服务器
./restart.sh
```

### 命令行选项

```bash
# 查看帮助
./target/release/uhorse --help

# 查看版本
./target/release/uhorse --version

# 使用自定义配置文件启动
./target/release/uhorse -c /path/to/config.toml run

# 设置日志级别
./target/release/uhorse -l debug run
```

### 健康检查

```bash
# 存活性检查
curl http://localhost:8080/health/live

# 就绪性检查
curl http://localhost:8080/health/ready

# 查看指标
curl http://localhost:8080/metrics
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
