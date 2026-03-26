#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }

mkdir -p data logs

if [ ! -x "./target/release/uhorse-hub" ] || [ ! -x "./target/release/uhorse-node" ]; then
    info "编译 uhorse-hub / uhorse-node release 二进制..."
    cargo build --release -p uhorse-hub -p uhorse-node
fi

if [ ! -f "hub.toml" ]; then
    cat > hub.toml <<'EOF'
[server]
host = "127.0.0.1"
port = 8765
EOF
    pass "已生成最小 hub.toml"
else
    info "保留现有 hub.toml"
fi

if [ ! -f "node.toml" ]; then
    cat > node.toml <<'EOF'
name = "local-node"
workspace_path = "."
require_git_repo = false

[connection]
hub_url = "ws://127.0.0.1:8765/ws"
reconnect_interval_secs = 5
heartbeat_interval_secs = 30
connect_timeout_secs = 10
max_reconnect_attempts = 10
EOF
    pass "已生成最小 node.toml"
else
    info "保留现有 node.toml"
fi

echo ""
echo "下一步："
echo "  1. 启动 Hub：./start.sh"
echo "  2. 启动 Node：make node-run"
echo "  3. 健康检查：curl http://127.0.0.1:8765/api/health"
echo "  4. 详细联调：查看 LOCAL_SETUP.md"
