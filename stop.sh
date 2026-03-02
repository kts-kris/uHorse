#!/bin/bash
# uHorse 停止脚本

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; }
info() { echo -e "${YELLOW}→${NC} $1"; }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  停止 uHorse 服务"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# 停止应用进程
info "查找 uHorse 进程..."
UHORSE_PID=$(pgrep -f "uhorse" | grep -v "grep\|start.sh" | head -1 || true)

if [ -n "$UHORSE_PID" ]; then
    info "停止 uHorse (PID: $UHORSE_PID)..."
    kill $UHORSE_PID
    pass "uHorse 已停止"
else
    info "未找到运行中的 uHorse 进程"
fi

# 停止依赖服务（可选）
echo ""
read -p "是否停止 PostgreSQL 和 Redis? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    info "停止依赖服务..."
    docker-compose down
    pass "依赖服务已停止"
else
    info "保持依赖服务运行"
fi

echo ""
pass "操作完成"
