#!/bin/bash
# uHorse 一键安装脚本
# 用于快速编译、配置和启动 uHorse

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_step() {
    echo -e "${PURPLE}▶ $1${NC}"
}

# 打印横幅
print_banner() {
    echo -e "${CYAN}"
    echo "╔════════════════════════════════════════════════╗"
    echo "║                                                ║"
    echo "║       🦄 uHorse AI Gateway                      ║"
    echo "║       一键安装脚本                              ║"
    echo "║                                                ║"
    echo "╚════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# 检查依赖
check_dependencies() {
    print_step "检查依赖..."

    # 检查 Rust
    if ! command -v rustc &> /dev/null; then
        print_error "未找到 Rust，请先安装 Rust"
        echo "请访问 https://rustup.rs/ 安装 Rust"
        exit 1
    fi
    print_success "Rust 已安装: $(rustc --version)"

    # 检查 Cargo
    if ! command -v cargo &> /dev/null; then
        print_error "未找到 Cargo"
        exit 1
    fi
    print_success "Cargo 已安装: $(cargo --version)"

    # 检查 OpenSSL（可选，用于生成 JWT 密钥）
    if command -v openssl &> /dev/null; then
        print_success "OpenSSL 已安装: $(openssl version)"
    else
        print_warning "未找到 OpenSSL，配置向导将无法自动生成 JWT 密钥"
    fi

    echo ""
}

# 编译项目
build_project() {
    print_step "编译项目..."
    print_info "这可能需要几分钟时间..."

    if cargo build --release; then
        print_success "编译成功"
    else
        print_error "编译失败"
        exit 1
    fi

    echo ""
}

# 运行配置向导
run_wizard() {
    print_step "启动配置向导..."

    if [ -f "config.toml" ] || [ -f ".env" ]; then
        print_warning "检测到现有配置文件"
        read -p "是否要重新配置？(y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_info "跳过配置向导，使用现有配置"
            return 0
        fi

        # 备份现有配置
        print_info "备份现有配置..."
        backup_dir="backup_$(date +%Y%m%d_%H%M%S)"
        mkdir -p "$backup_dir"
        [ -f "config.toml" ] && cp config.toml "$backup_dir/"
        [ -f ".env" ] && cp .env "$backup_dir/"
        print_success "配置已备份到: $backup_dir"
    fi

    # 运行配置向导
    ./target/release/uhorse wizard

    if [ $? -eq 0 ]; then
        print_success "配置完成"
    else
        print_error "配置失败"
        exit 1
    fi

    echo ""
}

# 创建必要的目录
create_directories() {
    print_step "创建必要的目录..."

    # 创建数据目录
    if [ ! -d "data" ]; then
        mkdir -p data
        print_success "创建数据目录: data/"
    fi

    # 创建日志目录
    if [ ! -d "logs" ]; then
        mkdir -p logs
        print_success "创建日志目录: logs/"
    fi

    echo ""
}

# 询问是否启动服务
ask_start_service() {
    print_step "安装完成！"

    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║                                                ║${NC}"
    echo -e "${GREEN}║     🎉 uHorse 安装完成！                      ║${NC}"
    echo -e "${GREEN}║                                                ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
    echo ""

    # 显示配置信息
    if [ -f "config.toml" ]; then
        print_info "配置文件: config.toml"
        host=$(grep -m 1 'host\s*=' config.toml | awk -F'"' '{print $2}')
        port=$(grep -m 1 'port\s*=' config.toml | awk -F'= ' '{print $2}' | tr -d ' ')
        print_info "服务地址: http://${host}:${port}"
    fi

    echo ""
    echo "下一步操作:"
    echo "  1️⃣  启动服务:     ./start.sh"
    echo "  2️⃣  查看日志:     tail -f logs/uhorse.log"
    echo "  3️⃣  检查状态:     curl http://localhost:8080/health/live"
    echo "  4️⃣  停止服务:     ./stop.sh"
    echo ""
    echo "📚 文档:"
    echo "  - 配置向导:     WIZARD.md"
    echo "  - 配置指南:     CONFIG.md"
    echo "  - API 使用:     API.md"
    echo "  - 通道集成:     CHANNELS.md"
    echo ""

    read -p "是否现在启动服务？(Y/n): " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
        print_info "启动服务..."
        ./start.sh
    else
        print_info "稍后可以使用 ./start.sh 启动服务"
    fi
}

# 主函数
main() {
    print_banner

    # 检查是否在项目根目录
    if [ ! -f "Cargo.toml" ]; then
        print_error "请在项目根目录运行此脚本"
        exit 1
    fi

    echo ""
    check_dependencies
    build_project
    create_directories
    run_wizard
    ask_start_service
}

# 运行主函数
main
