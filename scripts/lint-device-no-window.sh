#!/usr/bin/env bash
# Every subprocess spawn in the device crates must suppress the console window.
#
# The device crates are linked into a `windows_subsystem = "windows"` binary,
# which owns no console. When such a process spawns a console executable
# without CREATE_NO_WINDOW, Windows allocates a new console window for the
# child and hands it the foreground: a flash on every tool call, language
# server start or formatter run, and stolen keystrokes. The engine's tools,
# MCP transport, language servers and workspace formatters all spawn children,
# and every one of those sites is Windows-reachable.
#
# Scope: every `Command::new(` (std or tokio) under the `src` of a device
# crate (the DEVICE list of scripts/lint-device-closure.py). A spawn passes
# when `creation_flags` appears within three lines before or six lines after
# it, or when the builder is bound with `let mut NAME = Command::new(..)` and
# `NAME.creation_flags(` follows before the next spawn in the file. Anything
# else is outside what a line scan can classify; bind the builder to a name.
# `bin/bridge` has its own helper and its own gate (lint-bridge-no-window.sh).
#
#   scripts/lint-device-no-window.sh [--paths P...]
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
cd "$GATE_ROOT"

device=(
    crates/infra/device/src
    crates/infra/machine/src
    crates/domain/engine/src
    crates/domain/proxy/src
    crates/domain/hosts/src
    crates/domain/drive/src
    crates/domain/runner/src
    crates/app/harness/src
    crates/app/mobile/src
)
fail=0
scanned=0

for dir in "${device[@]}"; do
    [ -d "$dir" ] || { echo "lint-device-no-window: $dir is missing; the device list is out of step" >&2; exit 2; }
done

check_spawn() {
    local file="$1" line="$2"
    # Statements here are short; a six-line window forward plus three back covers
    # a builder chain.
    local start=$((line > 3 ? line - 3 : 1))
    if sed -n "${start},$((line + 6))p" "$file" | grep -q 'creation_flags'; then
        return 0
    fi
    local var next
    var=$(sed -n "${line}p" "$file" | sed -nE 's/.*let[[:space:]]+mut[[:space:]]+([a-z_][a-z0-9_]*)[[:space:]]*=.*Command::new\(.*/\1/p')
    if [ -n "$var" ]; then
        next=$(awk -v from="$line" 'NR > from && /Command::new\(/ { print NR; exit }' "$file")
        if sed -n "${line},${next:-\$}p" "$file" | grep -qE "(^|[^A-Za-z0-9_])${var}[[:space:]]*\.[[:space:]]*creation_flags\("; then
            return 0
        fi
    fi
    echo "ERROR: $file:$line spawns a process without CREATE_NO_WINDOW" >&2
    sed -n "${line}p" "$file" | sed 's/^/    /' >&2
    fail=1
}

while IFS=: read -r file line _; do
    [ -n "$line" ] || continue
    scanned=$((scanned + 1))
    check_spawn "$file" "$line"
done < <(grep -rnE '(process::)?Command::new\(' "${device[@]}" --include='*.rs' | gate_filter || true)

gate_pass_if_empty "lint-device-no-window" "$scanned"
if [ "$scanned" -eq 0 ]; then
    echo "lint-device-no-window: no spawn site found in the device crates; refusing to pass vacuously" >&2
    exit 2
fi

if [ "$fail" -ne 0 ]; then
    echo >&2
    echo "Set CREATE_NO_WINDOW through creation_flags on every spawn. A GUI-subsystem" >&2
    echo "process that spawns a console child without the flag pops a window and" >&2
    echo "steals focus." >&2
    exit 1
fi

echo "lint-device-no-window: OK (all $scanned spawn sites suppress the console)"
