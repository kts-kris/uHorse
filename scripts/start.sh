#!/bin/bash
# uHorse 本地启动脚本
# 用法: ./scripts/start.sh

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
echo "║     uHorse 本地开发启动                     ║"
echo "╚════════════════════════════════════════════════╝"

# 检查 Rust 工具链
section "1. 检查环境"
if ! command -v cargo &> /dev/null; then
    echo "错误: 未安装 cargo"
    echo "请安装 Rust: https://rustup.rs/"
    exit 1
fi
pass "Rust 工具链已安装"

# 检查 .env 文件
if [ ! -f .env ]; then
    section "2. 配置环境"
    info "创建 .env 文件..."
    cp .env.example .env
    info "已创建 .env 文件，请根据需要修改配置"
else
    pass "配置文件存在"
fi

# 加载环境变量
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | grep -v '^$' | xargs)
    pass "环境变量已加载"
fi

# 启动依赖服务
section "3. 启动依赖服务"
if command -v docker-compose &> /dev/null; then
    info "使用 Docker 启动 PostgreSQL 和 Redis..."
    docker-compose up -d postgres redis

    # 等待服务就绪
    info "等待服务启动..."
    sleep 3

    if docker-compose ps | grep -q "Up"; then
        pass "依赖服务已启动"
        docker-compose ps postgres redis
    else
        echo "警告: 依赖服务可能未正常启动"
    fi
else
    info "未安装 docker-compose，请确保 PostgreSQL 和 Redis 正在运行"
    info "安装方式: brew install docker-compose (macOS)"
fi

# 编译项目
section "4. 编译项目"
info "编译 uHorse..."
if cargo build --release 2>&1 | tail -n 5; then
    pass "编译成功"
else
    echo "错误: 编译失败"
    exit 1
fi

# 运行数据库迁移
section "5. 数据库迁移"
info "运行数据库迁移..."
if ./target/release/uhorse migrate 2>&1; then
    pass "数据库迁移完成"
else
    info "数据库迁移可能已执行或出错，继续启动..."
fi

# 启动应用
section "6. 启动应用"
echo ""
echo "╔════════════════════════════════════════════════╗"
echo "║     uHorse 正在启动                          ║"
echo "╚════════════════════════════════════════════════╝"
echo ""
echo "服务地址:"
echo "  - API:      http://localhost:8080"
echo "  - Health:   http://localhost:8080/health/live"
echo "  - Metrics:  http://localhost:8080/metrics"
echo "  - WebSocket: ws://localhost:8080/ws"
echo ""
echo "按 Ctrl+C 停止服务"
echo ""

# 启动应用
exec ./target/release/uhorse serve
