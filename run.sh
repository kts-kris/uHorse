#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

HOST="${UHORSE_HUB_HOST:-127.0.0.1}"
PORT="${UHORSE_HUB_PORT:-8765}"
LOG_LEVEL="${UHORSE_HUB_LOG_LEVEL:-info}"

if [ ! -x "./target/release/uhorse-hub" ]; then
    echo "[info] 编译 uhorse-hub release 二进制..."
    cargo build --release -p uhorse-hub
fi

echo "[info] 前台启动 uHorse Hub"
echo "[info] 健康检查：http://127.0.0.1:${PORT}/api/health"
echo "[info] WebSocket：ws://127.0.0.1:${PORT}/ws"
echo "[info] Node 请按 LOCAL_SETUP.md 或 make node-run 单独启动"

exec ./target/release/uhorse-hub --host "$HOST" --port "$PORT" --log-level "$LOG_LEVEL"
