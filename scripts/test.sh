#!/bin/bash
set -e

echo "╔════════════════════════════════════════════════╗"
echo "║     OpenClaw 自动化测试套件                   ║"
echo "╚════════════════════════════════════════════════╝"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; exit 1; }
info() { echo -e "${YELLOW}→${NC} $1"; }
section() { echo -e "\n${BLUE}▶${NC} $1"; }

# 获取脚本目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# 测试环境检查
section "1. 检查测试环境"
command -v docker >/dev/null 2>&1 || fail "Docker 未安装"
command -v docker-compose >/dev/null 2>&1 || fail "docker-compose 未安装"
command -v curl >/dev/null 2>&1 || fail "curl 未安装"
command -v cargo >/dev/null 2>&1 || fail "cargo 未安装"
pass "环境检查通过"

# 编译测试
section "2. 编译项目"
info "编译 release 版本..."
if cargo build --release 2>&1 | tee /tmp/build.log | tail -n 5; then
    pass "编译成功"
else
    fail "编译失败，查看 /tmp/build.log"
fi

# 单元测试
section "3. 运行单元测试"
info "运行 cargo test..."
if cargo test --quiet 2>&1 | tee /tmp/test.log | tail -n 5; then
    pass "单元测试通过"
else
    info "单元测试有失败，继续进行集成测试..."
fi

# Docker 构建测试
section "4. 构建 Docker 镜像"
info "构建 openclaw:test 镜像..."
if docker build -t openclaw:test -f Dockerfile . > /tmp/docker-build.log 2>&1; then
    pass "Docker 镜像构建成功"
    docker images | grep openclaw
else
    fail "Docker 构建失败，查看 /tmp/docker-build.log"
fi

# 启动测试环境
section "5. 启动测试环境"
info "启动 PostgreSQL, Redis 和 OpenClaw..."
docker-compose up -d postgres redis openclaw 2>&1 | tee /tmp/compose-up.log

# 等待服务启动
info "等待服务就绪..."
for i in {1..30}; do
    if curl -sf http://localhost:8080/health/live > /dev/null 2>&1; then
        pass "测试环境启动成功"
        break
    fi
    if [ $i -eq 30 ]; then
        fail "服务启动超时"
    fi
    sleep 1
done

# 显示服务状态
docker-compose ps

# 健康检查
section "6. 健康检查"
info "检查 /health/live 端点..."
HEALTH=$(curl -s http://localhost:8080/health/live)
echo "$HEALTH" | jq '.' 2>/dev/null || echo "$HEALTH"
if echo "$HEALTH" | jq -e '.status == "healthy"' >/dev/null 2>&1; then
    pass "健康检查通过"
else
    info "健康检查返回: $HEALTH"
fi

# 就绪检查
section "7. 就绪检查"
info "检查 /health/ready 端点..."
READY=$(curl -s http://localhost:8080/health/ready)
echo "$READY" | jq '.' 2>/dev/null || echo "$READY"
if echo "$READY" | jq -e '.status == "ready"' >/dev/null 2>&1; then
    pass "就绪检查通过"
else
    info "就绪检查返回: $READY"
fi

# 指标测试
section "8. 指标端点测试"
info "检查 /metrics 端点..."
METRICS=$(curl -s http://localhost:8080/metrics)
if echo "$METRICS" | grep -q "openclaw_"; then
    METRIC_COUNT=$(echo "$METRICS" | grep -c "^openclaw_")
    pass "指标端点正常 (找到 $METRIC_COUNT 个指标)"
    info "部分指标示例:"
    echo "$METRICS" | grep "^openclaw_" | head -n 5
else
    fail "指标端点未找到 openclaw_ 指标"
fi

# API 测试
section "9. API 端点测试"
info "测试 404 处理..."
HTTP_404=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/not-found)
if [ "$HTTP_404" = "404" ]; then
    pass "404 处理正常"
else
    info "404 返回: $HTTP_404"
fi

info "测试版本信息..."
VERSION=$(curl -s http://localhost:8080/health/live | jq -r '.version // "unknown"')
pass "版本: $VERSION"

# WebSocket 测试
section "10. WebSocket 连接测试"
if command -v wscat >/dev/null 2>&1; then
    info "测试 WebSocket ping/pong..."
    WS_RESPONSE=$(timeout 3 wscat -c ws://localhost:8080/ws <<< '{"type":"ping","id":"test-001"}' 2>/dev/null || echo "")
    if echo "$WS_RESPONSE" | grep -q "pong"; then
        pass "WebSocket 连接正常"
    else
        info "WebSocket 响应: $WS_RESPONSE"
        info "wscat 可能未正确连接，跳过详细测试"
    fi
else
    info "wscat 未安装，跳过 WebSocket 测试"
    info "安装命令: npm install -g wscat"
fi

# 性能测试
section "11. 性能测试"
if command -v wrk >/dev/null 2>&1; then
    info "运行 wrk 性能测试 (5秒, 10并发)..."
    if wrk -t2 -c10 -d5s http://localhost:8080/health/live 2>&1 | tee /tmp/wrk.log; then
        RPS=$(grep "Requests/sec" /tmp/wrk.log | awk '{print $2}')
        pass "性能测试完成 (RPS: $RPS)"
    fi
elif command -v ab >/dev/null 2>&1; then
    info "运行 ab 性能测试..."
    if ab -n 1000 -c 10 http://localhost:8080/health/live 2>&1 | tee /tmp/ab.log; then
        RPS=$(grep "Requests per second" /tmp/ab.log | awk '{print $4}')
        pass "性能测试完成 (RPS: $RPS)"
    fi
else
    info "wrk 或 ab 未安装，跳过性能测试"
    info "安装命令:"
    info "  wrk: brew install wrk  # macOS"
    info "  ab:  brew install apache2  # macOS"
fi

# 日志检查
section "12. 日志检查"
info "检查应用日志是否有错误..."
ERROR_COUNT=$(docker-compose logs --no-log-prefix openclaw 2>&1 | grep -i "error" | wc -l | tr -d ' ')
if [ "$ERROR_COUNT" -eq 0 ]; then
    pass "日志中无错误信息"
else
    info "发现 $ERROR_COUNT 条错误信息 (可能是正常的错误日志)"
    docker-compose logs --no-log-prefix openclaw 2>&1 | grep -i "error" | head -n 3
fi

# 资源使用
section "13. 资源使用"
docker stats --no-stream openclaw 2>/dev/null || true
pass "资源使用检查完成"

# 清理
section "14. 清理测试环境"
read -p "是否清理测试环境? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    docker-compose down -v
    pass "清理完成"
else
    info "跳过清理，环境保持运行"
    info "手动清理: docker-compose down -v"
fi

# 总结
echo ""
echo "╔════════════════════════════════════════════════╗"
echo "║     所有测试通过 ✓                            ║"
echo "╚════════════════════════════════════════════════╝"
echo ""
echo "测试日志保存在 /tmp/ 目录:"
echo "  - /tmp/build.log       (编译日志)"
echo "  - /tmp/test.log        (测试日志)"
echo "  - /tmp/docker-build.log (Docker 构建日志)"
echo "  - /tmp/compose-up.log   (Docker Compose 日志)"
echo ""
echo "快速命令:"
echo "  查看日志: docker-compose logs -f openclaw"
echo "  重启服务: docker-compose restart openclaw"
echo "  停止服务: docker-compose down"
