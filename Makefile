# uHorse Makefile - 快捷命令

.PHONY: help start stop restart dev test build clean install deps

# 默认目标
.DEFAULT_GOAL := help

# 颜色定义
GREEN  := $(shell tput -Txterm setaf 2)
YELLOW := $(shell tput -Txterm setaf 3)
WHITE  := $(shell tput -Txterm setaf 7)
RESET  := $(shell tput -Txterm sgr0)

# 帮助信息
help: ## 显示帮助信息
	@echo ''
	@echo '${GREEN}uHorse - 多渠道 AI 网关框架${RESET}'
	@echo ''
	@echo '使用方法:'
	@echo '  ${YELLOW}make${RESET} ${GREEN}<target>${RESET}'
	@echo ''
	@echo '可用命令:'
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*?##/ { printf "  ${YELLOW}%-15s${RESET} %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@echo ''

# ==================== 开发命令 ====================

start: ## 一键启动 uHorse（后台）
	@./start.sh

run: ## 前台启动 uHorse（开发模式）
	@./run.sh

start-bg: ## 后台启动 uHorse
	@./start.sh --daemon

stop: ## 停止 uHorse
	@./stop.sh

restart: ## 重启 uHorse
	@./restart.sh

dev: ## 开发模式启动 (热重载)
	@./scripts/dev.sh

# ==================== 构建命令 ====================

build: ## 编译项目
	@echo "${YELLOW}编译 uHorse...${RESET}"
	@cargo build --release

build-dev: ## 开发模式编译
	@echo "${YELLOW}开发模式编译...${RESET}"
	@cargo build

check: ## 快速检查 (不构建二进制)
	@echo "${YELLOW}检查代码...${RESET}"
	@cargo check

# ==================== 测试命令 ====================

test: ## 运行所有测试
	@echo "${YELLOW}运行测试...${RESET}"
	@cargo test --all

test-quick: ## 快速测试
	@./scripts/quick-test.sh

test-full: ## 完整测试
	@./scripts/test.sh

# ==================== 依赖管理 ====================

deps: ## 启动依赖服务 (PostgreSQL + Redis)
	@echo "${YELLOW}启动依赖服务...${RESET}"
	@docker-compose up -d postgres redis
	@echo "${GREEN}✓ 依赖服务已启动${RESET}"

deps-stop: ## 停止依赖服务
	@echo "${YELLOW}停止依赖服务...${RESET}"
	@docker-compose down
	@echo "${GREEN}✓ 依赖服务已停止${RESET}"

deps-status: ## 查看依赖服务状态
	@docker-compose ps

# ==================== 数据库命令 ====================

migrate: ## 运行数据库迁移
	@echo "${YELLOW}运行数据库迁移...${RESET}"
	@./target/release/uhorse migrate
	@echo "${GREEN}✓ 迁移完成${RESET}"

migrate-rollback: ## 回滚数据库迁移
	@echo "${YELLOW}回滚数据库迁移...${RESET}"
	@./target/release/uhorse migrate rollback

db-reset: ## 重置数据库 (危险!)
	@echo "${YELLOW}重置数据库...${RESET}"
	@./target/release/uhorse db:reset
	@echo "${GREEN}✓ 数据库已重置${RESET}"

# ==================== 清理命令 ====================

clean: ## 清理构建文件
	@echo "${YELLOW}清理构建文件...${RESET}"
	@cargo clean
	@echo "${GREEN}✓ 清理完成${RESET}

clean-all: ## 完整清理 (包括数据和日志)
	@echo "${YELLOW}完整清理...${RESET}"
	@cargo clean
	@rm -rf data/*.db data/*.log logs/*.log
	@echo "${GREEN}✓ 清理完成${RESET}"

# ==================== 安装命令 ====================

install: ## 安装 uHorse 到系统
	@echo "${YELLOW}安装 uHorse...${RESET}"
	@cargo install --path .
	@echo "${GREEN}✓ 安装完成${RESET}"
	@echo "运行: ${YELLOW}uhorse${RESET}"

# ==================== 日志命令 ====================

logs: ## 查看应用日志
	@tail -f data/uhorse.log 2>/dev/null || echo "日志文件不存在"

logs-deps: ## 查看依赖服务日志
	@docker-compose logs -f postgres redis

# ==================== 状态命令 ====================

status: ## 查看服务状态
	@echo "${YELLOW}uHorse 进程:${RESET}"
	@ps aux | grep -v grep | grep uhorse || echo "  未运行"
	@echo ""
	@echo "${YELLOW}依赖服务:${RESET}"
	@docker-compose ps 2>/dev/null || echo "  Docker Compose 未运行"

health: ## 健康检查
	@echo "${YELLOW}健康检查...${RESET}"
	@curl -s http://localhost:8080/health/live | jq '.' 2>/dev/null || curl -s http://localhost:8080/health/live

metrics: ## 查看指标
	@echo "${YELLOW}系统指标...${RESET}"
	@curl -s http://localhost:8080/metrics | grep -E "^uhorse_" | head -20

# ==================== 开发工具 ====================

watch: ## 监视文件变化并重新编译
	@echo "${YELLOW}启用文件监视...${RESET}"
	@cargo watch -x run

fmt: ## 格式化代码
	@echo "${YELLOW}格式化代码...${RESET}"
	@cargo fmt --all

fmt-check: ## 检查代码格式
	@cargo fmt --all -- --check

lint: ## 运行 linter
	@echo "${YELLOW}运行 Clippy...${RESET}"
	@cargo clippy --all --all-targets -- -D warnings

fix: ## 自动修复代码问题
	@echo "${YELLOW}自动修复...${RESET}"
	@cargo fix --allow-dirty --allow-staged

# ==================== Docker 命令 ====================

docker-up: ## 使用 Docker 启动所有服务
	@echo "${YELLOW}启动 Docker 环境...${RESET}"
	@docker-compose up -d
	@echo "${GREEN}✓ 所有服务已启动${RESET}"

docker-down: ## 停止 Docker 服务
	@echo "${YELLOW}停止 Docker 环境...${RESET}"
	@docker-compose down
	@echo "${GREEN}✓ 所有服务已停止${RESET}"

docker-logs: ## 查看 Docker 日志
	@docker-compose logs -f

docker-rebuild: ## 重新构建并启动 Docker
	@echo "${YELLOW}重新构建 Docker 镜像...${RESET}"
	@docker-compose up -d --build

# ==================== 更新命令 ====================

update: ## 更新依赖
	@echo "${YELLOW}更新依赖...${RESET}"
	@cargo update

upgrade: ## 升级依赖到最新版本
	@echo "${YELLOW}升级依赖...${RESET}"
	@cargo upgrade

# ==================== 文档命令 ====================

docs: ## 打开文档
	@echo "${YELLOW}打开文档...${RESET}"
	@cargo doc --open --no-deps

# ==================== 信息命令 ====================

info: ## 显示项目信息
	@echo "${GREEN}uHorse 项目信息${RESET}"
	@echo ""
	@echo "版本:      ${YELLOW}v1.0.0${RESET}"
	@echo "Rust 版本: ${YELLOW}$(shell rustc --version)${RESET}"
	@echo "Cargo 版本: ${YELLOW}$(shell cargo --version)${RESET}"
	@echo ""
	@echo "项目目录: ${YELLOW}$(shell pwd)${RESET}"
	@echo "配置文件: ${YELLOW}.env${RESET}"
	@echo ""
	@echo "服务地址:"
	@echo "  API:       ${YELLOW}http://localhost:8080${RESET}"
	@echo "  Health:    ${YELLOW}http://localhost:8080/health/live${RESET}"
	@echo "  Metrics:   ${YELLOW}http://localhost:8080/metrics${RESET}"
	@echo "  WebSocket: ${YELLOW}ws://localhost:8080/ws${RESET}"
