#!/bin/bash
# uHorse 快速设置脚本
# 使用默认配置快速启动 uHorse（适合本地开发测试）

set -e

# 颜色定义
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                ║${NC}"
echo -e "${BLUE}║       🦄 uHorse 快速设置                       ║${NC}"
echo -e "${BLUE}║       使用默认配置快速启动                     ║${NC}"
echo -e "${BLUE}║                                                ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""

# 检查是否已编译
if [ ! -f "./target/release/uhorse" ]; then
    echo -e "${YELLOW}⚠️  未找到编译后的二进制文件${NC}"
    echo "正在编译..."
    cargo build --release
    echo -e "${GREEN}✓ 编译完成${NC}"
    echo ""
fi

# 检查配置文件是否存在
if [ ! -f "config.toml" ]; then
    echo -e "${YELLOW}⚠️  未找到配置文件${NC}"
    echo "正在创建默认配置..."

    cat > config.toml << 'EOF'
# uHorse 默认配置
# 适用于本地开发和测试

[server]
host = "127.0.0.1"
port = 8080
max_connections = 1000

[channels]
enabled = []

[database]
path = "./data/uhorse.db"
pool_size = 10
conn_timeout = 30
wal_enabled = true
fk_enabled = true

[security]
token_expiry = 86400
refresh_token_expiry = 2592000
pairing_expiry = 300
approval_enabled = true
pairing_enabled = true

[logging]
level = "info"
format = "Pretty"
output = "Stdout"
ansi = true
file = true
line = true
target = true

[observability]
service_name = "uhorse"
tracing_enabled = true
metrics_enabled = true
metrics_port = 9090

[scheduler]
enabled = true
threads = 2
max_concurrent_jobs = 100

[tools]
sandbox_enabled = true
sandbox_timeout = 30
sandbox_max_memory = 512
EOF

    echo -e "${GREEN}✓ 配置文件已创建: config.toml${NC}"
    echo ""
fi

# 创建必要的目录
echo "创建必要的目录..."
mkdir -p data logs
echo -e "${GREEN}✓ 目录创建完成${NC}"
echo ""

# 显示完成信息
echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                                                ║${NC}"
echo -e "${GREEN}║     ✓ 快速设置完成！                          ║${NC}"
echo -e "${GREEN}║                                                ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo "默认配置:"
echo "  • 服务地址: http://127.0.0.1:8080"
echo "  • 数据库: SQLite (./data/uhorse.db)"
echo "  • 通道: 未启用"
echo ""
echo "下一步:"
echo "  1️⃣  启动服务:   ./start.sh"
echo "  2️⃣  健康检查:   curl http://127.0.0.1:8080/health/live"
echo "  3️⃣  配置通道:   ./target/release/uhorse wizard"
echo ""
echo "提示:"
echo "  • 使用 ./install.sh 进行完整配置"
echo "  • 查看 WIZARD.md 了解配置向导"
echo ""

# 询问是否启动
read -p "是否现在启动服务？ (Y/n): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
    ./start.sh
fi
