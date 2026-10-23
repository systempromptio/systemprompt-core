#!/usr/bin/env bash
# Gate: the gateway's bridge-facing paths are spelled once, in `ApiPaths`.
#
# `systemprompt_models::ApiPaths` (crates/shared/models/src/modules/api_paths.rs)
# holds the `/v1/bridge/*`, `/v1/auth/bridge/*`, `/otel`, `/mcp`,
# `/api/public/hooks` and `/api/v1/consumer/*` paths plus the two predicates
# (`is_otel`, `is_mcp`) that say what a prefix match means. The server
# registers its routes from those constants and the bridge builds its requests
# and classifies inbound routes from them, so a path typed by hand in either
# tree is a second table -- which is how `/mcp` and `/mcp/` came to disagree.
#
# Scope: `bin/bridge/src/**` and `crates/entry/api/src/routes/gateway/**`. A
# string literal there beginning `/v1/bridge`, `/v1/auth/bridge`, `/otel`,
# `/mcp`, `/api/public/hooks` or `/api/v1/` fails. Comments and `//!` docs are
# stripped first, so prose naming an endpoint is not a finding.
#
# Allowlist: `path|reason`. A file listed here may still spell those paths; an
# entry whose file holds no such literal fails so the list only shrinks.
set -euo pipefail

cd "$(dirname "$0")/.."

canonical="crates/shared/models/src/modules/api_paths.rs"
pattern='"/(v1/bridge|v1/auth/bridge|otel|mcp|api/public/hooks|api/v1/)[^"]*"'

allowlist=$(cat <<'LIST'
bin/bridge/src/gateway/identity.rs|owned by config-arch; removed when it lands
LIST
)

strip_comments() {
    perl -0pe 's{/\*.*?\*/}{}gs; s{^\s*//[^\n]*}{}mg; s{(^|[^:"])//[^\n]*}{$1}mg' "$1"
}

fail=0
declare -A allowed
while IFS='|' read -r path reason; do
    [ -n "$path" ] || continue
    [ -n "$reason" ] || { echo "lint-path-literals: allowlist entry for $path has no reason" >&2; exit 2; }
    [ -f "$path" ] || { echo "lint-path-literals: allowlisted file $path is gone; remove the entry" >&2; exit 2; }
    allowed["$path"]=0
done <<<"$allowlist"

while IFS= read -r file; do
    hits=$(strip_comments "$file" | grep -nE "$pattern" || true)
    [ -n "$hits" ] || continue
    if [ -n "${allowed[$file]+x}" ]; then
        allowed["$file"]=1
        continue
    fi
    while IFS= read -r hit; do
        echo "ERROR: $file:${hit%%:*} spells a gateway path by hand; use ApiPaths ($canonical)" >&2
        echo "    ${hit#*:}" >&2
    done <<<"$hits"
    fail=1
done < <(find bin/bridge/src crates/entry/api/src/routes/gateway -name '*.rs' | sort)

for path in "${!allowed[@]}"; do
    if [ "${allowed[$path]}" -eq 0 ]; then
        echo "ERROR: lint-path-literals: stale allowlist entry $path (no path literal left; remove it)" >&2
        fail=1
    fi
done

[ "$fail" -eq 0 ] || exit 1
echo "lint-path-literals: OK (bridge-facing paths spelled only in $canonical)"
