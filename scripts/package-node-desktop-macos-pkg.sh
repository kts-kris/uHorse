#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION="${1:-${UHORSE_VERSION:-$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(pkg["version"] for pkg in json.load(sys.stdin)["packages"] if pkg["name"] == "uhorse-node-desktop"))')}}"
TARGET="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
PACKAGE_ROOT="$PROJECT_ROOT/target/node-desktop-package"
PAYLOAD_DIR="$PACKAGE_ROOT/uhorse-node-desktop-$VERSION-$TARGET"
STAGING_DIR="$PACKAGE_ROOT/uhorse-node-desktop-$VERSION-$TARGET-macos-pkg"
PKG_PATH="$PACKAGE_ROOT/uhorse-node-desktop-$VERSION-$TARGET.pkg"
LAUNCHER_PATH="$STAGING_DIR/uHorse Node Desktop.command"

if [[ "$TARGET" != *apple-darwin* ]]; then
  printf 'unsupported target for macOS pkg: %s\n' "$TARGET" >&2
  exit 1
fi

if [ ! -d "$PAYLOAD_DIR" ]; then
  printf 'missing payload directory: %s\nrun ./scripts/package-node-desktop.sh first\n' "$PAYLOAD_DIR" >&2
  exit 1
fi

if ! command -v pkgbuild >/dev/null 2>&1; then
  printf 'pkgbuild is required to create a macOS .pkg\n' >&2
  exit 1
fi

rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"
cp -R "$PAYLOAD_DIR/." "$STAGING_DIR/"

cat > "$LAUNCHER_PATH" <<'EOF'
#!/bin/bash
set -euo pipefail

APP_ROOT="$(cd "$(dirname "$0")" && pwd)"
BASE_URL="http://127.0.0.1:8757"
CONFIG_DIR="$HOME/Library/Application Support/uHorse Node Desktop"
LOG_DIR="$HOME/Library/Logs/uHorse Node Desktop"
CONFIG_PATH="$CONFIG_DIR/node-desktop.toml"
LOG_PATH="$LOG_DIR/host.log"

mkdir -p "$CONFIG_DIR" "$LOG_DIR"

if ! curl -sf "$BASE_URL/api/settings/defaults" >/dev/null 2>&1; then
  nohup "$APP_ROOT/bin/uhorse-node-desktop" --config "$CONFIG_PATH" serve --listen "127.0.0.1:8757" >"$LOG_PATH" 2>&1 &
  for _ in {1..30}; do
    if curl -sf "$BASE_URL/api/settings/defaults" >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
fi

open "$BASE_URL/dashboard"
EOF
chmod +x "$LAUNCHER_PATH"

rm -f "$PKG_PATH"
pkgbuild \
  --root "$STAGING_DIR" \
  --identifier "com.uhorse.node-desktop" \
  --version "$VERSION" \
  --install-location "/Applications/uHorse Node Desktop" \
  "$PKG_PATH"

printf 'payload_dir=%s\n' "$PAYLOAD_DIR"
printf 'pkg=%s\n' "$PKG_PATH"
