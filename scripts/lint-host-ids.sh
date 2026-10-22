#!/usr/bin/env bash
# Gate: a bridge host id is spelled in exactly one place.
#
# `HostKind` (crates/shared/models/src/bridge/host.rs) is the vocabulary the
# gateway and the bridge share: `claude-code`, `claude-desktop`, `codex`,
# `codex-cli`, `opencode`, `opencode-cli`, `hermes`. Every other site derives
# the string through `HostKind::as_str`, and the web tree imports it from
# `bin/bridge/web/js/utils/host-ids.js`, the one hand-written mirror (a Rust
# test holds it equal to `HostKind::ALL`). A literal anywhere else is a second
# table that drifts when a host is added, which is how `codex` and
# `opencode-cli` came to exist nowhere but the manifest.
#
# Scope: `crates/**/src` and `bin/bridge/src` Rust sources and `bin/bridge/web`
# scripts. Tests, fixtures, SQL (whose CHECK sets are compared to the enum by
# `lint-sql-vocab`) and `.html` logo template ids are outside it.
#
# Allowlist: `path|literal|reason`, one per line. Reserved for a string that is
# not a host id but happens to spell one -- a binary name on PATH, or the
# `ClientKind` wire value the `ai_requests.client_kind` CHECK is built on. An
# entry whose line no longer matches fails the gate so the list only shrinks.
set -euo pipefail

cd "$(dirname "$0")/.."

ids='claude-code|claude-desktop|codex|codex-cli|opencode|opencode-cli|hermes'
pattern="\"(${ids})\""

canonical="crates/shared/models/src/bridge/host.rs"
mirror="bin/bridge/web/js/utils/host-ids.js"

allowlist=$(cat <<'LIST'
crates/shared/models/src/origin/mod.rs|claude-code claude-desktop codex opencode hermes|ClientKind wire values, CHECKed by ai_requests.client_kind; a wire client cannot tell app from CLI
crates/shared/models/src/origin/mod.rs|claude-code claude-desktop|User-Agent product tokens parsed by ClientKind::from_ua_product
crates/shared/models/src/origin/mod.rs|opencode|User-Agent product token parsed by ClientKind::from_ua_product
crates/app/agent-runs/src/host/codex_cli/mod.rs|codex|the `codex` executable the headless runner spawns
bin/bridge/src/integration/codex/config.rs|codex|the `codex` executable looked up on PATH
bin/bridge/src/integration/opencode/config.rs|opencode|the `opencode` executable looked up on PATH
bin/bridge/src/integration/hermes/config.rs|hermes|the `hermes` executable looked up on PATH
LIST
)

fail=0

hits=$(
    { grep -rnE --include='*.rs' "$pattern" crates/*/*/src bin/bridge/src 2>/dev/null | grep -v "^crates/tests/" || true
      grep -rnE --include='*.js' "$pattern" bin/bridge/web/js 2>/dev/null || true; } \
        | grep -v "^${canonical}:" | grep -v "^${mirror}:" || true
)

declare -A allowed_literals
while IFS='|' read -r path literals reason; do
    [ -n "$path" ] || continue
    [ -n "$reason" ] || { echo "lint-host-ids: allowlist entry for $path has no reason" >&2; exit 2; }
    for lit in $literals; do
        key="$path|$lit"
        allowed_literals["$key"]=0
    done
done <<<"$allowlist"

while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file=${hit%%:*}
    rest=${hit#*:}
    lineno=${rest%%:*}
    text=${rest#*:}
    for lit in $(grep -oE "$pattern" <<<"$text" | tr -d '"'); do
        key="$file|$lit"
        if [ -n "${allowed_literals[$key]+x}" ]; then
            allowed_literals["$key"]=$((allowed_literals["$key"] + 1))
        else
            echo "ERROR: $file:$lineno spells host id \"$lit\" outside $canonical" >&2
            echo "    $text" >&2
            fail=1
        fi
    done
done <<<"$hits"

for key in "${!allowed_literals[@]}"; do
    if [ "${allowed_literals[$key]}" -eq 0 ]; then
        echo "ERROR: lint-host-ids: stale allowlist entry $key (no matching literal; remove it)" >&2
        fail=1
    fi
done

if grep -rnE --include='*.rs' 'DesktopAndCli' crates/*/*/src bin/bridge/src >/dev/null 2>&1; then
    echo "ERROR: lint-host-ids: HostAppKind::DesktopAndCli reappeared; a host is one packaging" >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    echo "lint-host-ids: use HostKind::<Variant>.as_str() (Rust) or HOST_ID from $mirror (web)" >&2
    exit 1
fi
echo "lint-host-ids: OK (host ids spelled only in $canonical and $mirror)"
