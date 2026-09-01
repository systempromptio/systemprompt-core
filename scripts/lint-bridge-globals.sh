#!/usr/bin/env bash
# The bridge used to keep its service state in process statics: the proxy
# handle, the tokio runtime, the loopback secret, the MCP registry, the
# activity log, the gateway HTTP pool, the install id, the scheduler-status
# cache. A static can hold one value per process, which is why one test crate
# existed per proxy start outcome, and why a `sync` run in its own process
# wrote a managed-MCP policy from a registry it had never loaded. All of that
# now lives on `BridgeContext`, built once at the composition root and passed
# down. This gate keeps it there: a new `static X: OnceLock<..>` (or LazyLock,
# RwLock, Mutex, ArcSwap, Atomic*, Once) under bin/bridge/src is red unless it
# is listed below with the reason it is not service state.
set -euo pipefail

cd "$(dirname "$0")/.."

src="bin/bridge/src"

# file:NAME — reason
allow=(
    "obs/mod.rs:INIT — tracing's global dispatcher is itself set-once"
    "obs/mod.rs:GUARD — the tracing appender guard lives as long as the dispatcher"
    "obs/mod.rs:FILE_WRITER — the non-blocking writer the global dispatcher tees into"
    "brand.rs:BRAND — build identity, set by a white-label main before anything runs"
    "i18n.rs:CATALOG — read-only message catalogue, parsed once"
    "integration/registry.rs:REGISTRY — link-time inventory of host apps, sorted once"
    "host_sync/mod.rs:REGISTRY — link-time inventory of sync emitters, sorted once"
    "config/mod.rs:WARN_ONCE — rate-limits a warning, carries no state anyone reads"
    "auth/plugin_oauth/secret_store.rs:BACKEND — mirrors keyring_core::set_default_store, a set-once third-party global"
    "auth/plugin_oauth/secret_store.rs:MEMORY_SECRETS — the in-memory fallback behind that same set-once backend"
    "integration/codex_cli/install/mod.rs:SEQ — temp-file name uniqueness counter"
    "integration/hermes/install/mod.rs:SEQ — temp-file name uniqueness counter"
    "integration/opencode/install/mod.rs:SEQ — temp-file name uniqueness counter"
    "integration/claude_desktop/shared.rs:SEQ — temp-file name uniqueness counter"
)

is_allowed() {
    local key="$1" entry
    for entry in "${allow[@]}"; do
        [ "${entry%% — *}" = "$key" ] && return 0
    done
    return 1
}

fail=0
while IFS= read -r hit; do
    file=${hit%%:*}
    rest=${hit#*:}
    line=${rest%%:*}
    name=$(sed -E 's/^.*static ([A-Z_][A-Z0-9_]*):.*$/\1/' <<< "${rest#*:}")
    rel=${file#"$src"/}
    if ! is_allowed "$rel:$name"; then
        echo "lint-bridge-globals: $rel:$line -- static \`$name\` holds service state; build it in BridgeContext::start and inject it" >&2
        fail=1
    fi
done < <(git ls-files -co --exclude-standard "$src/*.rs" "$src/**/*.rs" | sort -u \
    | xargs grep -nE '^\s*(pub(\([a-z]+\))? )?static [A-Z_][A-Z0-9_]*: *(std::sync::)?(OnceLock|LazyLock|RwLock|Mutex|ArcSwap|Atomic[A-Za-z0-9]+|Once|OnceCell)\b' 2>/dev/null || true)

# An allowlist entry whose static no longer exists is stale; fail so it is
# removed rather than quietly covering a future reintroduction.
for entry in "${allow[@]}"; do
    key=${entry%% — *}
    file=${key%%:*}
    name=${key##*:}
    if ! grep -qE "static $name:" "$src/$file" 2>/dev/null; then
        echo "lint-bridge-globals: allowlist entry \`$key\` matches nothing -- remove it" >&2
        fail=1
    fi
done

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "lint-bridge-globals: OK (no unlisted process statics under $src)"
