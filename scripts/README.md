# OpenClaw 测试脚本

本目录包含 OpenClaw 项目的自动化测试脚本。

## 可用脚本

### 1. 快速测试 (quick-test.sh)

快速验证基础功能，适合开发时频繁使用。

```bash
./scripts/quick-test.sh
```

**测试内容**:
- 编译项目
- 运行单元测试
- 构建 Docker 镜像
- 启动服务
- 健康检查
- 指标检查

**预计耗时**: 1-2 分钟

### 2. 完整测试 (test.sh)

完整的测试套件，包括性能测试和资源使用检查。

```bash
./scripts/test.sh
```

**测试内容**:
- 环境检查
- 编译测试
- 单元测试
- Docker 构建
- 服务启动
- 健康检查
- 就绪检查
- 指标验证
- API 测试
- WebSocket 测试
- 性能测试 (wrk/ab)
- 日志检查
- 资源使用

**预计耗时**: 3-5 分钟

## 前置要求

### 必需

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 Docker
# macOS: 下载 Docker Desktop
# Linux: curl -fsSL https://get.docker.com | sh

# 安装 docker-compose
# macOS: 包含在 Docker Desktop 中
# Linux: sudo apt install docker-compose
```

### 可选 (用于更完整测试)

```bash
# WebSocket 测试工具
npm install -g wscat

# 性能测试工具
# macOS
brew install wrk
brew install apache2  # 包含 ab

# Linux
sudo apt install wrk apache2-utils
```

## 使用方法

### 首次测试

```bash
# 1. 克隆仓库
git clone https://github.com/openclaw/openclaw.git
cd openclaw

# 2. 运行快速测试
./scripts/quick-test.sh
```

### 开发时测试

```bash
# 修改代码后快速验证
./scripts/quick-test.sh

# 提交前完整测试
./scripts/test.sh
```

### 查看测试日志

```bash
# 测试脚本会将日志保存到 /tmp/
ls -la /tmp/*.log

# 查看编译日志
cat /tmp/build.log

# 查看测试日志
cat /tmp/test.log
```

## 测试场景

### 场景 1: 本地开发

```bash
# 1. 启动开发环境
docker-compose up -d postgres redis

# 2. 运行应用
cargo run --release

# 3. 运行测试
./scripts/quick-test.sh
```

### 场景 2: CI/CD

```bash
# 在 CI 环境运行完整测试
./scripts/test.sh

# 检查退出码
if [ $? -eq 0 ]; then
    echo "所有测试通过"
else
    echo "测试失败"
    exit 1
fi
```

### 场景 3: 性能基准

```bash
# 完整测试包含性能测试
./scripts/test.sh

# 查看性能结果
cat /tmp/wrk.log
# 或
cat /tmp/ab.log
```

## 手动测试

### 健康检查

```bash
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready
```

### 指标查看

```bash
curl http://localhost:8080/metrics | grep openclaw_
```

### WebSocket 测试

```bash
# 安装 wscat
npm install -g wscat

# 连接测试
wscat -c ws://localhost:8080/ws

# 发送 ping
> {"type":"ping","id":"test-001"}
```

### 性能测试

```bash
# 使用 wrk
wrk -t4 -c100 -d30s http://localhost:8080/health/live

# 使用 ab
ab -n 10000 -c 100 http://localhost:8080/health/live
```

## 故障排查

### 测试失败

```bash
# 查看详细日志
./scripts/test.sh 2>&1 | tee test-output.log

# 检查服务状态
docker-compose ps

# 查看服务日志
docker-compose logs openclaw

# 重启服务
docker-compose restart openclaw
```

### 端口占用

```bash
# 检查端口
lsof -i :8080

# 更改端口
export OPENCLAW_SERVER_PORT=8081
docker-compose up -d
```

### 清理环境

```bash
# 停止并清理
docker-compose down -v

# 清理镜像
docker rmi openclaw:test openclaw:latest

# 清理构建缓存
docker builder prune
```

## 添加自定义测试

创建新脚本 `scripts/custom-test.sh`:

```bash
#!/bin/bash
set -e

echo "运行自定义测试..."

# 添加你的测试逻辑
curl -f http://localhost:8080/api/custom || exit 1

echo "自定义测试通过"
```

赋予执行权限:
```bash
chmod +x scripts/custom-test.sh
```

## 更多信息

- [完整测试指南](../TESTING.md)
- [部署指南](../deployments/DEPLOYMENT.md)
- [项目进度](../PROGRESS.md)
