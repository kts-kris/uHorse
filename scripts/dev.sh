#!/bin/bash
# OpenClaw 开发模式启动脚本 (热重载)
# 用法: ./scripts/dev.sh

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
info() { echo -e "${YELLOW}→${NC} $1"; }
section() { echo -e "\n${BLUE}▶${NC} $1"; }

# 获取项目根目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo "╔════════════════════════════════════════════════╗"
echo "║     OpenClaw 开发模式 (热重载)                 ║"
echo "╚════════════════════════════════════════════════╝"

# 检查 cargo-watch
section "检查开发工具"
if ! command -v cargo-watch &> /dev/null; then
    info "安装 cargo-watch..."
    cargo install cargo-watch
fi
pass "开发工具就绪"

# 检查 .env 文件
section "配置环境"
if [ ! -f .env ]; then
    info "创建 .env 文件..."
    cp .env.example .env
fi

# 加载环境变量
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | grep -v '^$' | xargs)
    pass "环境变量已加载"
fi

# 启动依赖服务
section "启动依赖服务"
if command -v docker-compose &> /dev/null; then
    info "启动 PostgreSQL 和 Redis..."
    docker-compose up -d postgres redis
    sleep 3
    pass "依赖服务已启动"
else
    info "请确保 PostgreSQL 和 Redis 正在运行"
fi

# 运行数据库迁移
section "数据库迁移"
if [ -f target/debug/openclaw ]; then
    info "运行数据库迁移..."
    ./target/debug/openclaw migrate 2>/dev/null || true
fi

# 启动开发模式
section "启动开发服务器"
echo ""
echo "╔════════════════════════════════════════════════╗"
echo "║     开发模式启动中...                         ║"
echo "║     代码更改将自动重新编译                     ║"
echo "╚════════════════════════════════════════════════╝"
echo ""
echo "按 Ctrl+C 停止服务"
echo ""

# 启动热重载
exec cargo watch -x run
