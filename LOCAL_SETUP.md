# OpenClaw 本地开发启动指南

本指南介绍如何在本地开发环境中运行 OpenClaw，不使用 Docker 容器化应用。

## 目录

- [前置要求](#前置要求)
- [方法一：使用 Docker 运行依赖](#方法一使用-docker-运行依赖)
- [方法二：本地安装依赖服务](#方法二本地安装依赖服务)
- [启动应用](#启动应用)
- [开发工作流](#开发工作流)

---

## 前置要求

### 必需

```bash
# Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 验证安装
rustc --version
cargo --version
```

### 可选（热重载开发）

```bash
# 安装 cargo-watch
cargo install cargo-watch

# 安装 cargo-nextest（更快的测试运行器）
cargo install cargo-nextest
```

---

## 方法一：使用 Docker 运行依赖

**推荐方式**：应用本地运行，仅用 Docker 运行 PostgreSQL 和 Redis。

### 1. 启动依赖服务

```bash
# 仅启动数据库和缓存
docker-compose up -d postgres redis

# 验证服务状态
docker-compose ps

# 查看日志
docker-compose logs postgres
docker-compose logs redis
```

### 2. 配置环境变量

创建 `.env` 文件：

```bash
# 数据库
OPENCLAW_DATABASE_URL=postgresql://openclaw:password@localhost:5432/openclaw

# Redis
OPENCLAW_REDIS_URL=redis://localhost:6379

# 服务器
OPENCLAW_SERVER_HOST=127.0.0.1
OPENCLAW_SERVER_PORT=8080

# 日志
RUST_LOG=info

# 安全 (开发环境使用测试值)
OPENCLAW_JWT_SECRET=test-secret-for-development-only-do-not-use-in-production
```

### 3. 初始化数据库

```bash
# 运行数据库迁移
cargo run --release -- migrate

# 或使用开发模式
cargo run -- migrate
```

### 4. 启动应用

```bash
# 开发模式运行
cargo run

# 或使用 release 模式（更快）
cargo run --release

# 后台运行
cargo run --release > openclaw.log 2>&1 &
```

### 5. 验证

```bash
# 健康检查
curl http://localhost:8080/health/live

# 查看指标
curl http://localhost:8080/metrics

# WebSocket 测试
wscat -c ws://localhost:8080/ws
```

---

## 方法二：本地安装依赖服务

**完全脱离 Docker**：所有服务都在本地运行。

### 1. 安装 PostgreSQL

#### macOS

```bash
# 使用 Homebrew
brew install postgresql@14
brew services start postgresql@14

# 创建数据库
createdb openclaw

# 创建用户
psql -d postgres -c "CREATE USER openclaw WITH PASSWORD 'password';"
psql -d postgres -c "GRANT ALL PRIVILEGES ON DATABASE openclaw TO openclaw;"
```

#### Linux (Ubuntu/Debian)

```bash
# 安装 PostgreSQL
sudo apt update
sudo apt install postgresql-14 postgresql-contrib-14

# 启动服务
sudo systemctl start postgresql
sudo systemctl enable postgresql

# 创建数据库和用户
sudo -u postgres createdb openclaw
sudo -u postgres psql -c "CREATE USER openclaw WITH PASSWORD 'password';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE openclaw TO openclaw;"
```

#### 验证

```bash
# 连接测试
psql -U openclaw -d openclaw -h localhost

# 退出
\q
```

### 2. 安装 Redis

#### macOS

```bash
# 使用 Homebrew
brew install redis
brew services start redis

# 验证
redis-cli ping
# 应该返回: PONG
```

#### Linux (Ubuntu/Debian)

```bash
# 安装 Redis
sudo apt update
sudo apt install redis-server

# 启动服务
sudo systemctl start redis
sudo systemctl enable redis

# 验证
redis-cli ping
```

### 3. 配置环境变量

创建 `.env` 文件或设置环境变量：

```bash
# 在 shell 中设置
export OPENCLAW_DATABASE_URL="postgresql://openclaw:password@localhost:5432/openclaw"
export OPENCLAW_REDIS_URL="redis://localhost:6379"
export RUST_LOG="info"
export OPENCLAW_JWT_SECRET="test-secret-for-development-only"

# 或创建 .env 文件
cat > .env << EOF
OPENCLAW_DATABASE_URL=postgresql://openclaw:password@localhost:5432/openclaw
OPENCLAW_REDIS_URL=redis://localhost:6379
RUST_LOG=info
OPENCLAW_JWT_SECRET=test-secret-for-development-only
EOF
```

### 4. 加载环境变量

```bash
# 使用 direnv (推荐)
brew install direnv  # macOS
# 或
sudo apt install direnv  # Linux

# 在项目目录创建 .envrc
echo 'dotenv' > .envrc
direnv allow

# 或手动加载
source .env
# 或
export $(cat .env | xargs)
```

---

## 启动应用

### 开发模式（热重载）

```bash
# 使用 cargo-watch 自动重新编译
cargo watch -x run

# 指定端口
cargo watch -x 'run -- --port 8080'
```

### Release 模式

```bash
# 编译并运行
cargo run --release

# 指定配置文件
cargo run --release -- --config config.toml

# 指定端口
cargo run --release -- --port 8080
```

### 后台运行

```bash
# 后台运行并记录日志
cargo run --release > openclaw.log 2>&1 &

# 保存 PID
echo $! > openclaw.pid

# 查看日志
tail -f openclaw.log

# 停止服务
kill $(cat openclaw.pid)
```

---

## 开发工作流

### 1. 首次设置

```bash
# 1. 启动依赖
docker-compose up -d postgres redis
# 或本地启动 PostgreSQL 和 Redis

# 2. 加载环境变量
source .env
# 或
direnv allow

# 3. 运行数据库迁移
cargo run -- migrate

# 4. 启动应用
cargo run
```

### 2. 日常开发

```bash
# 启动依赖（如果未运行）
docker-compose up -d postgres redis

# 热重载开发
cargo watch -x run

# 或在另一个终端运行测试
cargo test
```

### 3. 调试

```bash
# 启用 debug 日志
RUST_LOG=debug cargo run

# 启用特定模块的详细日志
RUST_LOG=openclaw_gateway=debug,openclaw_tool=trace cargo run

# 使用 lldb 进行调试
rust-lldb -- target/debug/openclaw

# 使用 gdb 进行调试 (Linux)
rust-gdb -- target/debug/openclaw
```

### 4. 运行测试

```bash
# 所有测试
cargo test

# 特定模块
cargo test --package openclaw-gateway

# 带输出
cargo test -- --nocapture

# 并行测试
cargo test -- --test-threads=4
```

---

## CLI 参数

### 查看帮助

```bash
cargo run -- --help
```

### 常用命令

```bash
# 运行服务器
cargo run -- serve

# 指定端口
cargo run -- serve --port 8080

# 指定配置文件
cargo run -- serve --config config.toml

# 数据库迁移
cargo run -- migrate

# 回滚迁移
cargo run -- migrate rollback

# 数据库重置（危险！）
cargo run -- db:reset
```

---

## 故障排查

### 1. 数据库连接失败

```bash
# 检查 PostgreSQL 是否运行
ps aux | grep postgres

# 或
brew services list | grep postgresql  # macOS
sudo systemctl status postgresql     # Linux

# 检查连接
psql -U openclaw -d openclaw -h localhost
```

### 2. Redis 连接失败

```bash
# 检查 Redis 是否运行
redis-cli ping

# 或
brew services list | grep redis       # macOS
sudo systemctl status redis          # Linux

# 测试连接
redis-cli
> ping
> quit
```

### 3. 端口被占用

```bash
# 查看端口占用
lsof -i :8080

# 更改端口
export OPENCLAW_SERVER_PORT=8081
cargo run
```

### 4. 编译错误

```bash
# 清理构建缓存
cargo clean

# 重新构建
cargo build

# 更新依赖
cargo update
```

### 5. 日志查看

```bash
# 启用调试日志
RUST_LOG=debug cargo run

# 保存日志到文件
cargo run 2>&1 | tee openclaw.log

# 实时查看日志
tail -f openclaw.log
```

---

## 性能优化

### 1. 编译优化

```bash
# 使用 release 模式
cargo run --release

# 并行编译
export CARGO_BUILD_JOBS=8
cargo build --release
```

### 2. 开发优化

```bash
# 仅编译变化的包
cargo check

# 跳过未使用的依赖
cargo check --all-targets
```

---

## IDE 配置

### VS Code

安装扩展：
- rust-analyzer
- CodeLLDB
- Better TOML

配置 `.vscode/settings.json`:

```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "files.watcherExclude": {
    "**/target/**": true
  }
}
```

### IntelliJ IDEA

安装插件：
- Rust
- TOML

---

## 快速命令参考

```bash
# === 基础命令 ===
cargo run              # 开发模式运行
cargo run --release    # Release 模式运行
cargo test             # 运行测试
cargo build            # 编译项目

# === 开发工具 ===
cargo watch -x run     # 热重载开发
cargo check            # 快速检查
cargo clippy           # Lint 检查

# === 数据库 ===
cargo run -- migrate              # 运行迁移
cargo run -- migrate rollback     # 回滚迁移

# === 环境管理 ===
source .env           # 加载环境变量
docker-compose up -d postgres redis  # 启动依赖
docker-compose down   # 停止依赖
```

---

## 下一步

- [API 文档](API.md)
- [配置说明](CONFIG.md)
- [测试指南](TESTING.md)
