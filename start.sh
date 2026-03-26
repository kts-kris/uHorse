#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

PID_FILE=".uhorse-hub.pid"
LOG_FILE="logs/uhorse-hub.log"
HOST="${UHORSE_HUB_HOST:-127.0.0.1}"
PORT="${UHORSE_HUB_PORT:-8765}"
LOG_LEVEL="${UHORSE_HUB_LOG_LEVEL:-info}"
HEALTH_URL="http://127.0.0.1:${PORT}/api/health"

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }
fail() { echo "[error] $1"; exit 1; }

mkdir -p logs

if [ -f "$PID_FILE" ]; then
    PID="$(cat "$PID_FILE")"
    if kill -0 "$PID" 2>/dev/null; then
        pass "uHorse Hub 已在运行 (PID: $PID)"
        info "健康检查：$HEALTH_URL"
        exit 0
    fi
    rm -f "$PID_FILE"
fi

if [ ! -x "./target/release/uhorse-hub" ]; then
    info "编译 uhorse-hub release 二进制..."
    cargo build --release -p uhorse-hub
fi

info "启动 uHorse Hub..."
./target/release/uhorse-hub --host "$HOST" --port "$PORT" --log-level "$LOG_LEVEL" >"$LOG_FILE" 2>&1 &
PID=$!
echo "$PID" > "$PID_FILE"

if ! command -v curl >/dev/null 2>&1; then
    pass "uHorse Hub 已启动 (PID: $PID)"
    info "日志文件：$LOG_FILE"
    info "健康检查：$HEALTH_URL"
    exit 0
fi

for _ in {1..20}; do
    if curl -sf "$HEALTH_URL" >/dev/null 2>&1; then
        pass "uHorse Hub 已启动 (PID: $PID)"
        info "日志文件：$LOG_FILE"
        info "健康检查：$HEALTH_URL"
        info "Node 连接地址：ws://127.0.0.1:${PORT}/ws"
        exit 0
    fi

    if ! kill -0 "$PID" 2>/dev/null; then
        rm -f "$PID_FILE"
        tail -n 20 "$LOG_FILE" 2>/dev/null || true
        fail "uHorse Hub 启动失败"
    fi

    sleep 1
done

tail -n 20 "$LOG_FILE" 2>/dev/null || true
rm -f "$PID_FILE"
fail "uHorse Hub 启动超时"
