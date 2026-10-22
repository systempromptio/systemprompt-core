#!/usr/bin/env bash
# Gate: a systemprompt HTTP header name is spelled once.
#
# `systemprompt_identifiers::headers` (crates/shared/identifiers/src/headers.rs)
# names every header the platform propagates between its own processes, and
# `bin/bridge/src/headers.rs` names the ones that exist only between the
# bridge and the gateway. A literal that spells one of those values anywhere
# else is a second copy that stops matching the moment the constant moves, and
# a literal in the `x-systemprompt-` / `x-bridge-` namespace that no constant
# defines is a header with no reader or no writer.
#
# Scope: `crates/**/src` and `bin/bridge/src`. Vendor headers (`x-amz-*`,
# `x-stainless-*`, `x-goog-*`, `x-forwarded-for`, ...) and JSON keys that
# happen to start with `x-` are not header names of ours and are outside it.
#
# Allowlist: `path|literal|reason`, one per line. An entry whose line no longer
# matches fails the gate so the list only shrinks.
set -euo pipefail

cd "$(dirname "$0")/.."

definers=(
    crates/shared/identifiers/src/headers.rs
    bin/bridge/src/headers.rs
    crates/shared/models/src/bridge/manifest/mod.rs
)

allowlist=$(cat <<'LIST'
crates/shared/manifest/src/services/providers/headers.rs|x-api-key|owned by config-arch; removed when it lands
crates/entry/api/src/routes/gateway/messages/extract/headers.rs|x-api-key|owned by config-arch; removed when it lands
crates/domain/ai/src/services/personal_keys.rs|x-opencode-session|owned by config-arch; removed when it lands
crates/app/gateway/src/service/client_session.rs|x-opencode-session|owned by config-arch; removed when it lands
crates/app/gateway/src/service/mod.rs|x-systemprompt-request-id x-systemprompt-recovery-count|owned by config-arch; gateway response headers defined beside their writer until it lands
crates/shared/wire/src/anthropic/headers.rs|x-systemprompt-|a header-name prefix class for stripping, not a header name
LIST
)

defined=$(
    for f in "${definers[@]}"; do
        grep -hoE 'const [A-Z_]+: &str = "x-[a-z0-9-]+"' "$f" | grep -oE '"x-[a-z0-9-]+"' | tr -d '"'
    done | sort -u
)
defined_alt=$(paste -sd'|' <<<"$defined")
own_ns='x-(systemprompt|bridge)-[a-z0-9-]*'

declare -A allowed
while IFS='|' read -r path literals reason; do
    [ -n "$path" ] || continue
    [ -n "$reason" ] || { echo "lint-header-literals: allowlist entry for $path has no reason" >&2; exit 2; }
    for lit in $literals; do allowed["$path|$lit"]=0; done
done <<<"$allowlist"

is_definer() {
    local f
    for f in "${definers[@]}"; do [ "$1" = "$f" ] && return 0; done
    return 1
}

fail=0
while IFS= read -r file; do
    is_definer "$file" && continue
    while IFS= read -r hit; do
        [ -n "$hit" ] || continue
        lineno=${hit%%:*}
        text=${hit#*:}
        for lit in $(grep -oE '"x-[a-z0-9-]*"' <<<"$text" | tr -d '"'); do
            key="$file|$lit"
            if [ -n "${allowed[$key]+x}" ]; then
                allowed["$key"]=$((allowed["$key"] + 1))
                continue
            fi
            if grep -qxE "$defined_alt" <<<"$lit"; then
                echo "ERROR: $file:$lineno spells \"$lit\" by hand; use its constant" >&2
                echo "    $text" >&2
                fail=1
            elif grep -qxE "$own_ns" <<<"$lit"; then
                echo "ERROR: $file:$lineno names \"$lit\", which no header constant defines" >&2
                echo "    $text" >&2
                fail=1
            fi
        done
    done < <(grep -nE '"x-[a-z0-9-]*"' "$file" || true)
done < <(find crates/*/*/src bin/bridge/src -name '*.rs' -not -path 'crates/tests/*' | sort)

for key in "${!allowed[@]}"; do
    if [ "${allowed[$key]}" -eq 0 ]; then
        echo "ERROR: lint-header-literals: stale allowlist entry $key (no matching literal; remove it)" >&2
        fail=1
    fi
done

[ "$fail" -eq 0 ] || exit 1
echo "lint-header-literals: OK ($(wc -l <<<"$defined") header constants, no hand-spelled copies)"
