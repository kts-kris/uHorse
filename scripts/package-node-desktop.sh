#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

VERSION="${1:-${UHORSE_VERSION:-$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(pkg["version"] for pkg in json.load(sys.stdin)["packages"] if pkg["name"] == "uhorse-node-desktop"))')}}"
TARGET="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
BIN_SUFFIX=""
ARCHIVE_EXT="tar.gz"

case "$TARGET" in
  *windows*)
    BIN_SUFFIX=".exe"
    ARCHIVE_EXT="zip"
    ;;
esac

PACKAGE_ROOT="$PROJECT_ROOT/target/node-desktop-package"
PACKAGE_DIR="$PACKAGE_ROOT/uhorse-node-desktop-$VERSION-$TARGET"
WEB_DIR="$PACKAGE_DIR/web"
BIN_DIR="$PACKAGE_DIR/bin"
ARCHIVE_PATH="$PACKAGE_ROOT/uhorse-node-desktop-$VERSION-$TARGET.$ARCHIVE_EXT"

rm -rf "$PACKAGE_DIR"
mkdir -p "$WEB_DIR" "$BIN_DIR"

npm --prefix apps/node-desktop-web install
npm --prefix apps/node-desktop-web run build
cargo build --release --target "$TARGET" -p uhorse-node-desktop

cp "target/$TARGET/release/uhorse-node-desktop$BIN_SUFFIX" "$BIN_DIR/"
cp -R apps/node-desktop-web/dist/. "$WEB_DIR/"
cp README.md "$PACKAGE_DIR/" 2>/dev/null || true
cp CHANGELOG.md "$PACKAGE_DIR/" 2>/dev/null || true
cp LICENSE* "$PACKAGE_DIR/" 2>/dev/null || true

rm -f "$ARCHIVE_PATH"
if [ "$ARCHIVE_EXT" = "zip" ]; then
  (
    cd "$PACKAGE_ROOT"
    zip -qr "$ARCHIVE_PATH" "$(basename "$PACKAGE_DIR")"
  )
else
  tar -czf "$ARCHIVE_PATH" -C "$PACKAGE_ROOT" "$(basename "$PACKAGE_DIR")"
fi

printf 'package_dir=%s\n' "$PACKAGE_DIR"
printf 'archive=%s\n' "$ARCHIVE_PATH"
