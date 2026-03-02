#!/bin/bash
# uHorse 重启脚本

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
info() { echo -e "${YELLOW}→${NC} $1"; }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  重启 uHorse 服务"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# 停止服务
info "停止 uHorse..."
pkill -f "uhorse serve" 2>/dev/null || true
sleep 2
pass "已停止"

# 启动服务
info "启动 uHorse..."
./start.sh --daemon &
pass "重启完成"
