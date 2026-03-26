#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

HEALTH_URL="http://127.0.0.1:8765/api/health"
NODES_URL="http://127.0.0.1:8765/api/nodes"

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }
fail() { echo "[error] $1"; exit 1; }

command -v docker >/dev/null 2>&1 || fail "Docker 未安装"
command -v docker compose >/dev/null 2>&1 || fail "docker compose 未安装"
docker info >/dev/null 2>&1 || fail "Docker daemon 未运行"

cleanup() {
    docker compose stop uhorse-hub >/dev/null 2>&1 || true
    docker compose rm -f uhorse-hub >/dev/null 2>&1 || true
}
trap cleanup EXIT

info "编译当前主线 Hub + Node..."
cargo build --release -p uhorse-hub -p uhorse-node >/tmp/uhorse-quick-build.log 2>&1
pass "编译完成"

info "运行真实 roundtrip 回归..."
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture >/tmp/uhorse-quick-roundtrip.log 2>&1
pass "roundtrip 回归通过"

info "检查当前 workspace 是否可作为 Node 工作区..."
cargo run --quiet --release -p uhorse-node -- check --workspace . >/tmp/uhorse-quick-node-check.log 2>&1
pass "Node workspace 检查通过"

info "构建 Hub Docker 镜像..."
docker build -t uhorse-hub:latest -f Dockerfile . >/tmp/uhorse-quick-docker.log 2>&1
pass "Docker 镜像构建完成"

info "启动 Docker Hub smoke 环境..."
docker compose up -d uhorse-hub >/tmp/uhorse-quick-compose.log 2>&1

for _ in {1..20}; do
    if curl -sf "$HEALTH_URL" | grep -q 'healthy'; then
        pass "Hub /api/health 可访问"
        break
    fi
    sleep 1
done

if ! curl -sf "$HEALTH_URL" | grep -q 'healthy'; then
    docker compose logs --no-log-prefix uhorse-hub | tail -n 50 || true
    fail "Hub /api/health 检查失败"
fi

curl -sf "$NODES_URL" >/tmp/uhorse-quick-nodes.json 2>&1
pass "Hub /api/nodes 可访问"

echo ""
echo "快速测试完成。关键日志："
echo "  /tmp/uhorse-quick-build.log"
echo "  /tmp/uhorse-quick-roundtrip.log"
echo "  /tmp/uhorse-quick-node-check.log"
echo "  /tmp/uhorse-quick-docker.log"
echo "  /tmp/uhorse-quick-compose.log"
