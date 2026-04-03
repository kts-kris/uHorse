# uHorse Makefile - Hub / Node 主线快捷命令

.PHONY: help start run start-bg stop restart dev quick-setup build build-hub build-node check test test-workspace test-quick test-full roundtrip auth-smoke skill-install-smoke node-run node-check desktop-web-build desktop-build desktop-package desktop-package-macos desktop-package-windows desktop-smoke desktop-installer-smoke deps deps-stop deps-status docker-build docker-up docker-down docker-logs clean install logs status health watch fmt fmt-check lint info

.DEFAULT_GOAL := help

GREEN  := $(shell tput -Txterm setaf 2)
YELLOW := $(shell tput -Txterm setaf 3)
WHITE  := $(shell tput -Txterm setaf 7)
RESET  := $(shell tput -Txterm sgr0)

help: ## 显示帮助信息
	@echo ''
	@echo '${GREEN}uHorse Hub / Node 主线命令${RESET}'
	@echo ''
	@echo '使用方法:'
	@echo '  ${YELLOW}make${RESET} ${GREEN}<target>${RESET}'
	@echo ''
	@echo '可用命令:'
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*?##/ { printf "  ${YELLOW}%-15s${RESET} %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ''

start: ## 后台启动 Hub（127.0.0.1:8765）
	@./start.sh

run: ## 前台启动 Hub
	@./run.sh

start-bg: ## start 的别名
	@./start.sh

stop: ## 停止后台 Hub
	@./stop.sh

restart: ## 重启后台 Hub
	@./restart.sh

dev: ## 热重载启动 Hub
	@./scripts/dev.sh

quick-setup: ## 生成最小 hub.toml / node.toml
	@./quick-setup.sh

build: ## 编译当前主线 Hub + Node 二进制
	@cargo build --release -p uhorse-hub -p uhorse-node

build-hub: ## 编译 Hub 二进制
	@cargo build --release -p uhorse-hub

build-node: ## 编译 Node 二进制
	@cargo build --release -p uhorse-node

check: ## 快速检查 Hub + Node
	@cargo check -p uhorse-hub -p uhorse-node

test: ## 运行当前主线包级测试
	@cargo test -p uhorse-node-runtime
	@cargo test -p uhorse-hub

test-workspace: ## 运行工作区测试
	@cargo test --workspace

test-quick: ## 运行快速主线回归
	@./scripts/quick-test.sh

test-full: ## 运行完整主线回归
	@./scripts/test.sh

roundtrip: ## 运行真实 Hub-Node roundtrip 回归
	@cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture

auth-smoke: ## 运行 JWT node_id 拒绝回归
	@cargo test -p uhorse-hub test_local_hub_rejects_node_with_mismatched_auth_token -- --nocapture

skill-install-smoke: ## 运行 Agent Browser Skill 安装自动化回归
	@cargo test -p uhorse-hub test_agent_browser_natural_language_install_flow_returns_chinese_hint -- --nocapture

node-run: ## 启动 Node（需要已准备 node.toml）
	@cargo run --release -p uhorse-node -- --config node.toml --log-level info

node-check: ## 检查当前工作区是否可作为 Node workspace
	@cargo run --release -p uhorse-node -- check --workspace .

desktop-web-build: ## 构建 Node Desktop Web 前端
	@npm --prefix apps/node-desktop-web install
	@npm --prefix apps/node-desktop-web run build

desktop-build: ## 编译 Node Desktop 宿主
	@cargo build --release -p uhorse-node-desktop

desktop-package: ## 打包 Node Desktop 宿主 + Web 静态资源 archive
	@./scripts/package-node-desktop.sh

desktop-package-macos: ## 基于现有 payload 生成 macOS pkg
	@./scripts/package-node-desktop-macos-pkg.sh

desktop-package-windows: ## 基于现有 payload 生成 Windows installer
	@./scripts/package-node-desktop-windows-installer.ps1

desktop-smoke: ## 运行 Node Desktop archive API + 静态资源 smoke
	@./scripts/desktop-smoke.sh

desktop-installer-smoke: ## 运行安装后目录的 Node Desktop smoke（需 INSTALL_ROOT=...）
	@./scripts/desktop-installer-smoke.sh "$(INSTALL_ROOT)"

deps: ## 启动 PostgreSQL + Redis 依赖
	@docker compose up -d postgres redis

deps-stop: ## 停止 Docker 依赖
	@docker compose down

deps-status: ## 查看 Docker 服务状态
	@docker compose ps

docker-build: ## 构建 Hub Docker 镜像
	@docker build -t uhorse-hub:latest -f Dockerfile .

docker-up: ## 启动 Hub + 依赖 Docker 环境
	@docker compose up -d uhorse-hub postgres redis

docker-down: ## 停止 Docker 环境
	@docker compose down

docker-logs: ## 查看 Hub Docker 日志
	@docker compose logs -f uhorse-hub

clean: ## 清理构建产物
	@cargo clean

install: ## 本地快速准备当前主线
	@./install.sh

logs: ## 查看 Hub 日志
	@tail -f logs/uhorse-hub.log 2>/dev/null || echo "logs/uhorse-hub.log 不存在"

status: ## 查看 Hub 进程与健康状态
	@if [ -f .uhorse-hub.pid ]; then echo "Hub PID: $$(cat .uhorse-hub.pid)"; else echo "Hub PID: 未记录"; fi
	@pgrep -fl "uhorse-hub" || echo "未找到 uhorse-hub 进程"
	@curl -sf http://127.0.0.1:8765/api/health || echo "Hub 未响应 /api/health"

health: ## 调用当前主线健康检查
	@curl -s http://127.0.0.1:8765/api/health

watch: ## 监视 Hub 代码并热重载
	@./scripts/dev.sh

fmt: ## 格式化代码
	@cargo fmt --all

fmt-check: ## 检查代码格式
	@cargo fmt --all -- --check

lint: ## 对 Hub + Node 运行 clippy
	@cargo clippy -p uhorse-hub -p uhorse-node --all-targets -- -D warnings

info: ## 显示当前主线入口
	@echo "${GREEN}uHorse 主线信息${RESET}"
	@echo ""
	@echo "Hub:        ${YELLOW}http://127.0.0.1:8765${RESET}"
	@echo "Health:     ${YELLOW}http://127.0.0.1:8765/api/health${RESET}"
	@echo "WebSocket:  ${YELLOW}ws://127.0.0.1:8765/ws${RESET}"
	@echo "Node 启动:  ${YELLOW}cargo run --release -p uhorse-node -- --config node.toml --log-level info${RESET}"
	@echo "文档:       ${YELLOW}LOCAL_SETUP.md / TESTING.md${RESET}"
