#!/usr/bin/env bash
# Wrap the systemprompt-bridge binary in a macOS .app bundle (branded "Cowork").
#
# Usage: bin/bridge/scripts/make-mac-app.sh [--target <triple>]
#
# Reads version from bin/bridge/Cargo.toml, picks the matching release binary,
# generates an .icns from bin/bridge/assets/window-icon-1024.png, and emits
# bin/bridge/target/<triple>/release/SystempromptCowork.app
# (or bin/bridge/target/release/SystempromptCowork.app when no target is given).
set -euo pipefail

cd "$(dirname "$0")/../../.."  # repo root

TARGET=""
if [[ "${1:-}" == "--target" ]]; then
    TARGET="${2:?--target requires a value}"
fi

CRATE_DIR="bin/bridge"
ASSETS="$CRATE_DIR/assets"
PLIST_TEMPLATE="$CRATE_DIR/macos/Info.plist"

if [[ -n "$TARGET" ]]; then
    BIN="$CRATE_DIR/target/$TARGET/release/systemprompt-bridge"
    OUT_DIR="$CRATE_DIR/target/$TARGET/release"
else
    BIN="$CRATE_DIR/target/release/systemprompt-bridge"
    OUT_DIR="$CRATE_DIR/target/release"
fi

if [[ ! -f "$BIN" ]]; then
    echo "binary not found at $BIN — build it first (just build-bridge${TARGET:+ $TARGET})" >&2
    exit 1
fi

VERSION="$(awk -F'"' '/^version/ { print $2; exit }' "$CRATE_DIR/Cargo.toml")"
APP="$OUT_DIR/SystempromptCowork.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RES_DIR="$CONTENTS/Resources"

rm -rf "$APP"
mkdir -p "$MACOS_DIR" "$RES_DIR"

cp "$BIN" "$MACOS_DIR/systemprompt-cowork"
chmod +x "$MACOS_DIR/systemprompt-cowork"

sed "s/__VERSION__/$VERSION/g" "$PLIST_TEMPLATE" > "$CONTENTS/Info.plist"

# Regenerate the brand-shaped 1024 source + 44px template tray icon, then
# render the .iconset and pack it into AppIcon.icns. swift ships with macOS.
swift "$CRATE_DIR/scripts/make-icons.swift" "$ASSETS"

ICON_SRC="$ASSETS/window-icon-1024.png"
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 64 128 256 512; do
    sips -z $size $size "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png"   >/dev/null
    sips -z $((size*2)) $((size*2)) "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns -o "$RES_DIR/AppIcon.icns" "$ICONSET"
rm -rf "$(dirname "$ICONSET")"

echo "built: $APP (v$VERSION)"
echo "run with: open '$APP'  or  '$MACOS_DIR/systemprompt-cowork' gui"
