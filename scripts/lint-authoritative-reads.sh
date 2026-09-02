#!/usr/bin/env bash
# Security-critical lookups must read the primary, never a replica.
#
# `database_url` may point at a regional standby that lags the primary by
# seconds. A session, token, revocation or ban written to the primary and
# looked up on the replica a moment later is simply absent there, which turns
# a fresh login into a 401 and a revocation into a window of continued access.
# The repositories below therefore run their lookups on `write_pool`; a
# `self.pool` read in any of them is only allowed when listed here as a
# deliberately non-authoritative query.
set -uo pipefail

SENSITIVE_FILES=(
    crates/domain/analytics/src/repository/session/mod.rs
    crates/domain/users/src/repository/api_key.rs
    crates/domain/users/src/repository/user/session.rs
    crates/domain/users/src/repository/banned_ip/queries.rs
    crates/domain/users/src/repository/device_cert.rs
    crates/domain/mcp/src/repository/session/mod.rs
    crates/domain/oauth/src/repository/bridge_session.rs
    crates/domain/oauth/src/repository/setup_token.rs
    crates/domain/oauth/src/repository/oauth/refresh_token/ops.rs
    crates/domain/oauth/src/repository/oauth/jti_revocation.rs
    crates/domain/oauth/src/repository/oauth/auth_code/mod.rs
    crates/domain/oauth/src/repository/webauthn.rs
    crates/domain/oauth/src/repository/oauth/id_jag_replay.rs
    crates/domain/oauth/src/repository/oauth/state_binding.rs
    crates/domain/oauth/src/repository/exchange_code.rs
)

# file:function pairs whose replica read is acceptable — listings and
# analytics that tolerate replication lag. Every entry must still exist.
ALLOWLIST=(
    "crates/domain/analytics/src/repository/session/mod.rs:find_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:list_active_by_user"
    "crates/domain/analytics/src/repository/session/mod.rs:count_inactive"
    "crates/domain/analytics/src/repository/session/mod.rs:find_recent_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:count_sessions_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:get_endpoint_sequence"
    "crates/domain/analytics/src/repository/session/mod.rs:get_request_timestamps"
    "crates/domain/analytics/src/repository/session/mod.rs:get_total_content_pages"
    "crates/domain/analytics/src/repository/session/mod.rs:get_session_for_behavioral_analysis"
    "crates/domain/analytics/src/repository/session/mod.rs:has_analytics_events"
    "crates/domain/analytics/src/repository/session/mod.rs:count_unique_ips_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:count_engagement_events_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:get_session_starts_by_fingerprint"
    "crates/domain/analytics/src/repository/session/mod.rs:get_session_velocity"
    "crates/domain/analytics/src/repository/session/mod.rs:count_sessions_missing_geo"
    "crates/domain/users/src/repository/api_key.rs:list_api_keys_for_user"
    "crates/domain/users/src/repository/user/session.rs:list_sessions"
    "crates/domain/users/src/repository/user/session.rs:list_recent_sessions"
)

READ_RE='\.(fetch_one|fetch_optional|fetch_all|fetch_scalar)\((&\*self\.pool\b|self\.pool_ref\(\)|self\.pool\.as_ref\(\)|&self\.pool\b|pool\.as_ref\(\))'
# The `let pool = &self.pool;` idiom hides the read pool behind a local.
LOCAL_RE='let pool = &self\.pool;'

allowed() {
    local key="$1"
    for entry in "${ALLOWLIST[@]}"; do
        [ "$entry" = "$key" ] && return 0
    done
    return 1
}

fail=0
for entry in "${ALLOWLIST[@]}"; do
    file="${entry%%:*}"; fn="${entry##*:}"
    if ! grep -qE "fn ${fn}\b" "$file" 2>/dev/null; then
        echo "stale allowlist entry: $entry (function not found)"
        fail=1
    fi
done

for file in "${SENSITIVE_FILES[@]}"; do
    [ -f "$file" ] || { echo "missing sensitive file: $file"; fail=1; continue; }
    current_fn=""
    lineno=0
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        if [[ "$line" =~ fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
            current_fn="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ $READ_RE ]] || [[ "$line" =~ $LOCAL_RE ]]; then
            if ! allowed "$file:$current_fn"; then
                echo "$file:$lineno: replica read in security-critical repository (fn $current_fn); use the write pool or allowlist it in scripts/lint-authoritative-reads.sh"
                fail=1
            fi
        fi
    done < "$file"
done

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "lint-authoritative-reads: ok"
