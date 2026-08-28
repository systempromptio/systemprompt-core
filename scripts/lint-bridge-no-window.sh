#!/usr/bin/env bash
# Every Windows-reachable subprocess spawn in the bridge must suppress the
# console window.
#
# The bridge is a `windows_subsystem = "windows"` binary, so it owns no console.
# When such a process spawns a console executable without CREATE_NO_WINDOW,
# Windows allocates a brand-new console window for the child and hands it the
# foreground. `tray::refresh` called `schtasks /Query` on every 30-second probe
# tick, so the shipped app flashed a console into the user's face twice a minute
# and stole their keystrokes. The flag now lives in one place --
# `winproc::no_window` / `winproc::silenced_command` -- and this gate keeps every
# spawn site routed through it.
#
# Scope: Windows-only modules (`windows.rs`, `*/windows/*.rs`), plus any spawn of
# a Windows system executable anywhere in the tree -- which is how the
# cross-platform `cmd /C start` arms in `gui/window/mod.rs` and
# `auth/providers/session.rs` are caught. A spawn of a runtime-computed path
# (the post-update relaunch) is outside what a grep can classify.
set -euo pipefail

cd "$(dirname "$0")/.."

bridge="bin/bridge/src"
win_exes='cmd|powershell|pwsh|schtasks|icacls|reg|reg\.exe|cmd\.exe|wmic|tasklist'
fail=0

check_spawn() {
    local file="$1" line="$2"
    # Statements here are short; a six-line window forward plus three back covers
    # every builder chain in the tree.
    local start=$((line > 3 ? line - 3 : 1))
    if sed -n "${start},$((line + 6))p" "$file" \
        | grep -qE 'winproc::(no_window|silenced_command|reg_command)|creation_flags'; then
        return 0
    fi
    echo "ERROR: $file:$line spawns a process without CREATE_NO_WINDOW" >&2
    sed -n "${line}p" "$file" | sed 's/^/    /' >&2
    fail=1
}

# 1. Windows-only modules: every spawn in them is Windows-reachable by definition.
while IFS= read -r file; do
    # winproc.rs defines the helper, so it is the one file allowed the raw flag.
    [ "$file" = "$bridge/winproc.rs" ] && continue
    while IFS=: read -r line _; do
        [ -n "$line" ] && check_spawn "$file" "$line"
    done < <(grep -nE '(process::)?Command::new\(' "$file" || true)
done < <(find "$bridge" -name 'windows.rs' -o -path '*/windows/*.rs' | sort)

# 2. Windows system executables, wherever they are spawned from.
while IFS=: read -r file line _; do
    [ -n "$line" ] || continue
    [ "$file" = "$bridge/winproc.rs" ] && continue
    case "$file" in *windows.rs | */windows/*.rs) continue ;; esac
    check_spawn "$file" "$line"
done < <(grep -rnE "(process::)?Command::new\(\"($win_exes)\"\)" "$bridge" --include='*.rs' || true)

if [ "$fail" -ne 0 ]; then
    echo >&2
    echo "Route the spawn through crate::winproc::no_window(&mut cmd) (or" >&2
    echo "silenced_command / reg_command). A GUI-subsystem process that spawns a" >&2
    echo "console child without the flag pops a window and steals focus." >&2
    exit 1
fi

echo "bridge no-window: all Windows spawn sites suppress the console"
