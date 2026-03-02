#!/bin/bash
# uHorse 简化启动脚本
set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
info() { echo -e "${YELLOW}→${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; exit 1; }

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  uHorse 一键启动"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# 1. 确保配置文件存在
if [ ! -f config.toml ]; then
    info "创建配置文件..."
    cat > config.toml << 'EOF'
[server]
host = "127.0.0.1"
port = 8080

[channels]
enabled = []

[database]
path = "./data/uhorse.db"

[security]
jwt_secret = "dev-secret"
token_expiry = 86400
EOF
    pass "配置文件已创建"
fi

# 2. 确保数据目录存在
mkdir -p data logs
pass "数据目录已就绪"

# 3. 检查二进制文件
if [ ! -f target/release/uhorse ]; then
    info "编译项目..."
    cargo build --release 2>&1 | tail -3
fi
pass "二进制文件就绪"

# 4. 停止已有进程
pkill -f "target/release/uhorse" 2>/dev/null || true
sleep 1

# 5. 启动服务
info "启动 uHorse..."
./target/release/uhorse > uhorse.log 2>&1 &
UHORSE_PID=$!
echo "  PID: $UHORSE_PID"

# 6. 等待服务就绪
info "等待服务启动..."
for i in {1..15}; do
    if curl -sf http://localhost:8080/health/live > /dev/null 2>&1; then
        echo ""
        pass "uHorse 启动成功！"
        echo ""
        break
    fi
    if [ $i -eq 15 ]; then
        echo ""
        fail "启动超时！查看日志:"
        echo ""
        tail -20 uhorse.log
    fi
    echo -n "."
    sleep 1
done

# 7. 显示状态
echo -e "${CYAN}服务信息:${NC}"
HEALTH=$(curl -s http://localhost:8080/health/live)
echo "$HEALTH" | jq '.' 2>/dev/null || echo "$HEALTH"
echo ""

echo -e "${CYAN}服务地址:${NC}"
echo "  🌐  API:       http://localhost:8080"
echo "  ❤️  Health:    http://localhost:8080/health/live"
echo "  📊 Metrics:   http://localhost:8080/metrics"
echo "  🔌 WebSocket: ws://localhost:8080/ws"
echo ""

echo -e "${CYAN}常用命令:${NC}"
echo "  查看日志:   tail -f uhorse.log"
echo "  停止服务:   pkill -f uhorse"
echo "  重启服务:   ./start.sh"
echo "  前台运行:  ./run.sh"
echo ""

echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  🎉 uHorse 运行中 (PID: $UHORSE_PID)${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
