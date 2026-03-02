#!/bin/bash
# uHorse 前台启动脚本（开发模式）
# 用法: ./run.sh

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  uHorse 前台启动 (开发模式)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# 检查配置文件
if [ ! -f config.toml ]; then
    echo -e "${YELLOW}创建默认配置...${NC}"
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
fi

# 检查数据目录
mkdir -p data logs

# 检查二进制文件
if [ ! -f target/release/uhorse ]; then
    echo -e "${YELLOW}首次编译...${NC}"
    cargo build --release
fi

echo ""
echo -e "${GREEN}启动 uHorse...${NC}"
echo ""
echo -e "${CYAN}服务地址:${NC}"
echo "  http://localhost:8080"
echo "  http://localhost:8080/health/live"
echo ""
echo -e "${YELLOW}按 Ctrl+C 停止${NC}"
echo ""

# 直接运行（前台）
exec ./target/release/uhorse
