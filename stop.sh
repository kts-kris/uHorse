#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

PID_FILE=".uhorse-hub.pid"

stop_pid() {
    local pid="$1"

    if ! kill -0 "$pid" 2>/dev/null; then
        return 0
    fi

    kill "$pid"
    for _ in {1..10}; do
        if ! kill -0 "$pid" 2>/dev/null; then
            return 0
        fi
        sleep 1
    done

    kill -9 "$pid" 2>/dev/null || true
}

if [ -f "$PID_FILE" ]; then
    PID="$(cat "$PID_FILE")"
    stop_pid "$PID"
    rm -f "$PID_FILE"
    echo "[ok] 已停止 uHorse Hub (PID: $PID)"
    exit 0
fi

PID="$(pgrep -f "target/release/uhorse-hub|cargo run --release -p uhorse-hub" | head -n 1 || true)"
if [ -n "$PID" ]; then
    stop_pid "$PID"
    echo "[ok] 已停止 uHorse Hub (PID: $PID)"
    exit 0
fi

echo "[info] 未找到运行中的 uHorse Hub"
