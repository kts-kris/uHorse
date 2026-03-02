# uHorse 安装指南

## 目录

- [系统要求](#系统要求)
- [安装方式](#安装方式)
- [安装验证](#安装验证)
- [常见问题](#常见问题)

---

## 系统要求

### 最低要求

- **操作系统**: Linux、macOS 或 Windows (WSL2)
- **Rust**: 1.70 或更高版本
- **内存**: 512 MB 可用内存
- **磁盘**: 100 MB 可用空间

### 推荐配置

- **CPU**: 2 核心或更多
- **内存**: 2 GB 或更多
- **磁盘**: SSD 存储

### 依赖项

uHorse 需要以下依赖：

- **Rust 工具链**: 用于编译项目
- **OpenSSL**: 可选，用于生成安全密钥

---

## 安装方式

### 方式一：一键安装脚本（推荐）⭐

**适用场景：**
- 首次安装
- 生产环境部署
- 需要完整配置

**步骤：**

```bash
# 1. 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 2. 运行一键安装脚本
./install.sh
```

**脚本执行流程：**

1. ✅ 检查依赖（Rust、Cargo、OpenSSL）
2. 🔨 编译项目（Release 模式）
3. 📁 创建必要目录（data/、logs/）
4. 🧙 启动配置向导
5. 💾 生成配置文件（config.toml、.env）
6. 🚀 可选：立即启动服务

**优势：**
- 全自动化，无需手动操作
- 包含完整的配置向导
- 支持所有功能配置
- 自动备份现有配置

---

### 方式二：快速设置脚本

**适用场景：**
- 快速本地测试
- 开发环境
- 体验 uHorse 功能

**步骤：**

```bash
# 1. 克隆仓库
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs

# 2. 运行快速设置脚本
./quick-setup.sh
```

**脚本执行流程：**

1. 🔨 编译项目（如需要）
2. 📝 创建默认配置文件
3. 📁 创建必要目录
4. 🚀 可选：立即启动服务

**默认配置：**
- 服务地址：`http://127.0.0.1:8080`
- 数据库：SQLite（`./data/uhorse.db`）
- 通道：未启用
- 日志级别：info

**优势：**
- 快速启动，无需交互
- 适合本地开发
- 可后续通过配置向导修改

---

### 方式三：手动安装

**适用场景：**
- 需要自定义编译选项
- 了解项目结构
- 集成到现有系统

**步骤：**

#### 1. 安装 Rust

```bash
# 使用 rustup 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 重新加载环境
source $HOME/.cargo/env
```

#### 2. 克隆仓库

```bash
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs
```

#### 3. 编译项目

```bash
# Debug 模式（编译快）
cargo build

# Release 模式（性能优化）
cargo build --release
```

#### 4. 创建配置文件

**使用配置向导（推荐）：**

```bash
./target/release/uhorse wizard
```

**或手动创建：**

创建 `config.toml`：

```toml
[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = []

[database]
path = "./data/uhorse.db"

[security]
token_expiry = 86400
```

创建 `.env`：

```bash
RUST_LOG=info
```

#### 5. 创建必要目录

```bash
mkdir -p data logs
```

#### 6. 启动服务

```bash
# 使用启动脚本
./start.sh

# 或直接运行
./target/release/uhorse run
```

---

### 方式四：Docker 安装

**适用场景：**
- 容器化部署
- 隔离运行环境
- CI/CD 流程

**步骤：**

#### 1. 使用 Docker Compose

```bash
# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f uhorse

# 停止服务
docker-compose down
```

#### 2. 单独构建和运行

```bash
# 构建镜像
docker build -t uhorse:latest .

# 运行容器
docker run -d \
  --name uhorse \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/config.toml:/app/config.toml \
  uhorse:latest
```

---

## 安装验证

### 1. 检查二进制文件

```bash
./target/release/uhorse --version
```

**预期输出：**
```
uhorse 0.1.0
```

### 2. 检查配置文件

```bash
# 检查配置文件是否存在
ls -la config.toml .env

# 查看配置内容
cat config.toml
```

### 3. 启动服务

```bash
./start.sh
```

### 4. 健康检查

```bash
# 存活性检查
curl http://localhost:8080/health/live

# 预期输出
# {"status":"healthy","version":"0.1.0"}

# 就绪性检查
curl http://localhost:8080/health/ready

# 预期输出
# {"status":"ready","version":"0.1.0"}
```

### 5. 查看日志

```bash
tail -f logs/uhorse.log
```

---

## 配置

### 使用配置向导

```bash
./target/release/uhorse wizard
```

### 手动编辑配置

编辑 `config.toml`：

```bash
vi config.toml
```

编辑 `.env`：

```bash
vi .env
```

### 配置文件优先级

1. 命令行参数（最高优先级）
2. 环境变量（.env 文件）
3. 配置文件（config.toml）
4. 默认值（最低优先级）

---

## 卸载

### 停止服务

```bash
./stop.sh
```

### 删除文件

```bash
# 删除二进制文件
rm -rf target/

# 删除配置文件
rm config.toml .env

# 删除数据文件
rm -rf data/ logs/

# 删除备份文件（可选）
rm -rf backup_*/
```

---

## 常见问题

### Q: 编译失败怎么办？

**A: 检查 Rust 版本：**

```bash
rustc --version
```

确保使用 Rust 1.70 或更高版本。如果版本过低，请更新：

```bash
rustup update
```

### Q: 找不到 OpenSSL 错误

**A: 安装 OpenSSL：**

**macOS:**
```bash
brew install openssl
```

**Ubuntu/Debian:**
```bash
sudo apt-get install libssl-dev pkg-config
```

**CentOS/RHEL:**
```bash
sudo yum install openssl-devel
```

### Q: 端口被占用

**A: 修改配置文件中的端口号：**

编辑 `config.toml`：

```toml
[server]
port = 8081  # 改为其他端口
```

### Q: 如何升级到最新版本？

**A: 拉取最新代码并重新编译：**

```bash
git pull origin main
cargo build --release
./stop.sh
./start.sh
```

### Q: 如何备份配置？

**A: 在运行配置向导前手动备份：**

```bash
mkdir backup
cp config.toml backup/
cp .env backup/
```

或使用一键安装脚本，它会自动备份。

### Q: 如何查看详细日志？

**A: 设置日志级别为 debug：**

编辑 `.env`：

```bash
RUST_LOG=debug
```

或使用命令行参数：

```bash
./target/release/uhorse -l debug run
```

---

## 下一步

安装完成后，请参考以下文档：

- [配置向导指南](WIZARD.md) - 交互式配置说明
- [配置指南](CONFIG.md) - 完整配置参考
- [API 使用指南](API.md) - API 文档
- [通道集成指南](CHANNELS.md) - 多通道配置
- [部署指南](deployments/DEPLOYMENT.md) - 生产环境部署
