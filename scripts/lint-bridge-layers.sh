#!/usr/bin/env bash
# The bridge is one crate, so `just lint-layers` (which walks the cargo
# dependency graph) cannot see its module structure — and it had cycles:
# integration ⇄ sync, integration ⇄ install, and a host installer that
# reached up into `gui` to pop a dialog. This gate declares the module order
# and fails on any `crate::<module>` reference that points upward.
#
# Order is bottom-up. A module may name any module listed BEFORE it, never one
# after it. Leaves come first and reference nothing but each other.
set -euo pipefail

cd "$(dirname "$0")/.."

src="bin/bridge/src"

order="brand ids basedirs fsutil hash i18n sysproc winproc verdict user_alert stdio obs activity progress web_assets ipc_types cowork_compat single_instance schedule probe_cache
config
buildinfo window_state
gateway
mcp_registry
auth
validate update
proxy_probe
proxy
context
host_sync
install
integration
sync
wire
gui
dev_preview
cli"

rank_of() {
    local i=0 line
    while IFS= read -r line; do
        for m in $line; do
            if [ "$m" = "$1" ]; then echo "$i"; return 0; fi
        done
        i=$((i + 1))
    done <<< "$order"
    return 1
}

# A `crate::name` token is a module reference only if `name` is a module.
# Everything else is a macro (`$crate::register_host_app!`) or an item.
is_module() {
    [ -f "$src/$1.rs" ] || [ -d "$src/$1" ]
}

fail=0
while IFS= read -r file; do
    [ -f "$file" ] || continue
    rel=${file#"$src"/}
    self=${rel%%/*}
    self=${self%.rs}
    [ "$self" = "lib" ] || [ "$self" = "main" ] && continue
    self_rank=$(rank_of "$self") || { echo "lint-bridge-layers: module \`$self\` is not in the order list -- add it" >&2; exit 2; }
    # every `crate::<mod>` / `$crate::<mod>` token in the file, deduplicated
    while IFS= read -r target; do
        [ -z "$target" ] && continue
        [ "$target" = "$self" ] && continue
        is_module "$target" || continue
        target_rank=$(rank_of "$target") || { echo "lint-bridge-layers: module \`$target\` is not in the order list -- add it" >&2; exit 2; }
        if [ "$target_rank" -gt "$self_rank" ]; then
            line=$(grep -nE "crate::${target}\b" "$file" | head -1 | cut -d: -f1)
            echo "lint-bridge-layers: $rel:$line -- \`$self\` references \`$target\`, which sits above it" >&2
            fail=1
        fi
    done < <(grep -oE '\$?crate::[a-z_]+' "$file" | sed -E 's/^\$?crate:://' | sort -u)
done < <(git ls-files -co --exclude-standard "$src/*.rs" "$src/**/*.rs" | sort -u)

if [ "$fail" -ne 0 ]; then
    echo "lint-bridge-layers: move the shared piece below both modules, or pass it in from the composition root" >&2
    exit 1
fi
echo "lint-bridge-layers: OK (no upward module references)"
