#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }
fail() { echo "[error] $1"; exit 1; }

command -v rustc >/dev/null 2>&1 || fail "未找到 rustc，请先安装 Rust"
command -v cargo >/dev/null 2>&1 || fail "未找到 cargo"
pass "Rust 环境检查通过"

info "编译当前主线 Hub + Node..."
cargo build --release -p uhorse-hub -p uhorse-node
pass "release 编译完成"

./quick-setup.sh

echo ""
echo "安装完成。推荐顺序："
echo "  1. ./start.sh"
echo "  2. make node-run"
echo "  3. curl http://127.0.0.1:8765/api/health"
echo "  4. 查看 LOCAL_SETUP.md / TESTING.md"
