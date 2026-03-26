#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

LISTEN="${DESKTOP_SMOKE_LISTEN:-127.0.0.1:8757}"
BASE_URL="http://$LISTEN"
RUNTIME_DIR="$(mktemp -d)"
WEB_DIR="$RUNTIME_DIR/web"
CONFIG_PATH="$RUNTIME_DIR/node-desktop.toml"
LOG_PATH="$RUNTIME_DIR/node-desktop.log"
PID=""

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }
fail() { echo "[error] $1"; exit 1; }

cleanup() {
  if [ -n "$PID" ] && kill -0 "$PID" >/dev/null 2>&1; then
    kill "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$RUNTIME_DIR"
}
trap cleanup EXIT

command -v curl >/dev/null 2>&1 || fail "curl 未安装"
command -v python3 >/dev/null 2>&1 || fail "python3 未安装"

info "构建 Node Desktop Web 与宿主..."
npm --prefix apps/node-desktop-web install >/dev/null
npm --prefix apps/node-desktop-web run build >/dev/null
cargo build --release -p uhorse-node-desktop >/dev/null
pass "构建完成"

mkdir -p "$WEB_DIR"
cp -R apps/node-desktop-web/dist/. "$WEB_DIR/"
cat > "$CONFIG_PATH" <<EOF
name = "Desktop Smoke"
workspace_path = "$PROJECT_ROOT"
require_git_repo = false

[connection]
hub_url = "ws://localhost:8765/ws"
EOF

info "启动 Node Desktop 本地宿主..."
UHORSE_NODE_DESKTOP_WEB_DIR="$WEB_DIR" target/release/uhorse-node-desktop --config "$CONFIG_PATH" serve --listen "$LISTEN" >"$LOG_PATH" 2>&1 &
PID=$!

for _ in {1..30}; do
  if curl -sf "$BASE_URL/api/settings/defaults" >/dev/null 2>&1; then
    pass "宿主 API 已启动"
    break
  fi
  sleep 1
done

curl -sf "$BASE_URL/api/settings/defaults" >/dev/null || {
  cat "$LOG_PATH" >&2 || true
  fail "宿主 API 启动失败"
}

python3 - <<'PY' "$BASE_URL"
import json, sys, urllib.request
base = sys.argv[1]
for path in [
    '/api/settings/defaults',
    '/api/settings/capabilities',
    '/api/workspace/status',
    '/api/runtime/status',
    '/api/versioning/summary',
]:
    with urllib.request.urlopen(base + path) as response:
        payload = json.load(response)
        assert payload['success'] is True, path
PY
pass "关键 API smoke 通过"

INDEX_HTML="$(curl -sf "$BASE_URL/")"
case "$INDEX_HTML" in
  *"id=\"root\""*) pass "静态首页可访问" ;;
  *) fail "静态首页内容不符合预期" ;;
esac

APP_HTML="$(curl -sf "$BASE_URL/dashboard")"
case "$APP_HTML" in
  *"id=\"root\""*) pass "前端路由回退可访问" ;;
  *) fail "前端路由回退不可用" ;;
esac

echo ""
echo "Node Desktop smoke 完成。日志：$LOG_PATH"
