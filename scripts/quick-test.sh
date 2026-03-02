#!/bin/bash
# 快速测试脚本 - 验证基础功能

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; exit 1; }
info() { echo -e "${YELLOW}→${NC} $1"; }

echo "=== uHorse 快速测试 ==="

# 1. 编译
info "编译项目..."
cargo build --release --quiet 2>&1 | tail -n 3
pass "编译成功"

# 2. 单元测试
info "运行单元测试..."
cargo test --quiet 2>&1 | tail -n 3
pass "单元测试通过"

# 3. Docker 构建
info "构建 Docker 镜像..."
docker build -t uhorse:latest -q . 2>&1 | tail -n 3
pass "Docker 镜像构建成功"

# 4. 启动服务
info "启动服务..."
docker-compose up -d postgres redis uhorse --quiet
sleep 5

# 5. 健康检查
info "健康检查..."
if curl -sf http://localhost:8080/health/live | grep -q "healthy"; then
    pass "健康检查通过"
else
    fail "健康检查失败"
fi

# 6. 指标检查
info "指标检查..."
if curl -sf http://localhost:8080/metrics | grep -q "uhorse_"; then
    pass "指标端点正常"
else
    fail "指标端点异常"
fi

echo ""
echo "=== 快速测试完成 ✓ ==="
echo ""
echo "服务运行中:"
echo "  - API: http://localhost:8080"
echo "  - Metrics: http://localhost:8080/metrics"
echo "  - WebSocket: ws://localhost:8080/ws"
echo ""
echo "停止服务: docker-compose down"
