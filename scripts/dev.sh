#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

if ! command -v cargo-watch >/dev/null 2>&1; then
    echo "[info] 安装 cargo-watch..."
    cargo install cargo-watch
fi

echo "[info] 热重载目标：uhorse-hub"
echo "[info] 健康检查：http://127.0.0.1:8765/api/health"
echo "[info] Node 请按 LOCAL_SETUP.md 单独启动"

exec cargo watch -x 'run -p uhorse-hub -- --host 127.0.0.1 --port 8765 --log-level debug'
