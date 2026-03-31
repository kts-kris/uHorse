#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

INSTALL_ROOT="${1:-${DESKTOP_INSTALL_ROOT:-}}"
LISTEN="${DESKTOP_SMOKE_LISTEN:-127.0.0.1:8757}"
BASE_URL="http://$LISTEN"
RUNTIME_DIR="$(mktemp -d)"
CONFIG_PATH="$RUNTIME_DIR/node-desktop.toml"
LOG_PATH="$RUNTIME_DIR/node-desktop-installer.log"
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

[ -n "$INSTALL_ROOT" ] || fail "用法：./scripts/desktop-installer-smoke.sh <install-root>"
BIN_PATH="$INSTALL_ROOT/bin/uhorse-node-desktop"

[ -x "$BIN_PATH" ] || fail "未找到安装后的宿主二进制：$BIN_PATH"
[ -f "$INSTALL_ROOT/web/index.html" ] || fail "未找到安装后的 web/index.html"
[ -d "$INSTALL_ROOT/web/assets" ] || fail "未找到安装后的 web/assets"
command -v curl >/dev/null 2>&1 || fail "curl 未安装"
command -v python3 >/dev/null 2>&1 || fail "python3 未安装"

cat > "$CONFIG_PATH" <<EOF
name = "Desktop Installer Smoke"
workspace_path = "$PROJECT_ROOT"
require_git_repo = false

[connection]
hub_url = "ws://localhost:8765/ws"
EOF

info "启动安装后的 Node Desktop 宿主..."
"$BIN_PATH" --config "$CONFIG_PATH" serve --listen "$LISTEN" >"$LOG_PATH" 2>&1 &
PID=$!

for _ in {1..30}; do
  if curl -sf "$BASE_URL/api/settings/defaults" >/dev/null 2>&1; then
    pass "安装后的宿主 API 已启动"
    break
  fi
  sleep 1
done

curl -sf "$BASE_URL/api/settings/defaults" >/dev/null || {
  cat "$LOG_PATH" >&2 || true
  fail "安装后的宿主 API 启动失败"
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
pass "安装后的关键 API smoke 通过"

INDEX_HTML="$(curl -sf "$BASE_URL/")"
case "$INDEX_HTML" in
  *"id=\"root\""*) pass "安装后的静态首页可访问" ;;
  *) fail "安装后的静态首页内容不符合预期" ;;
esac

APP_HTML="$(curl -sf "$BASE_URL/dashboard")"
case "$APP_HTML" in
  *"id=\"root\""*) pass "安装后的前端路由回退可访问" ;;
  *) fail "安装后的前端路由回退不可用" ;;
esac

echo ""
echo "Node Desktop installer smoke 完成。日志：$LOG_PATH"
