#!/usr/bin/env bash
# Build systemprompt-bridge for Windows from WSL and deploy it to the Windows
# Desktop for testing.
#
# Why GNU and not MSVC: the canonical release target is
# x86_64-pc-windows-msvc, but cross-building MSVC from WSL needs the MSVC CRT
# + lld-link (cargo-xwin), which in turn needs `sudo apt install lld`. Until
# that toolchain is provisioned, we build x86_64-pc-windows-gnu, which links
# WebView2Loader.dll dynamically — so we ship that DLL next to the .exe (MSVC
# resolves it via an import lib and needs no loose DLL). The DLL is the
# official Microsoft loader vendored by the webview2-com-sys crate.
#
# Usage: bin/bridge/scripts/deploy-windows.sh [DEST_DIR]
#   DEST_DIR defaults to the current user's Windows Desktop.
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"
BRIDGE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-/mnt/c/Users/ejb50/Desktop}"
PS="/mnt/c/windows/System32/WindowsPowerShell/v1.0/powershell.exe"

echo ">> building ($TARGET, release)"
cargo build --manifest-path "$BRIDGE_DIR/Cargo.toml" --release --target "$TARGET"

EXE="$BRIDGE_DIR/target/$TARGET/release/systemprompt-bridge.exe"
# The webview2-com-sys build hash changes between builds — resolve the newest
# x64 loader under the release build tree.
DLL="$(find "$BRIDGE_DIR/target/$TARGET/release/build" -ipath '*webview2-com-sys-*/out/x64/WebView2Loader.dll' -printf '%T@ %p\n' \
        | sort -nr | head -1 | cut -d' ' -f2-)"

[ -f "$EXE" ] || { echo "!! missing $EXE" >&2; exit 1; }
[ -f "$DLL" ] || { echo "!! could not locate WebView2Loader.dll under the build tree" >&2; exit 1; }

echo ">> stopping any running bridge on Windows"
"$PS" -NoProfile -Command "Get-Process systemprompt-bridge -ErrorAction SilentlyContinue | Stop-Process -Force" || true
sleep 1

echo ">> deploying to $DEST"
cp -f "$EXE" "$DEST/systemprompt-bridge.exe"
cp -f "$DLL" "$DEST/WebView2Loader.dll"

echo ">> done"
ls -la "$DEST/systemprompt-bridge.exe" "$DEST/WebView2Loader.dll"
