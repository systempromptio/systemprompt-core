#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# Match sqlx::query( and sqlx::query_{as,scalar,file,file_as,file_scalar,with,...}(
pattern='sqlx::query[a-z_]*(::<[^>]*>)?\('

# Each entry is a path: a directory (trailing `/`) exempts the tree under it, a
# file exempts that file. A stale carve-out is removed, not kept: an entry
# whose path is gone, or that no longer holds a runtime query, fails the gate.
allowlist=(
    'crates/infra/database/src/admin/'
    'crates/infra/database/src/services/postgres/'
    # Test crates keep no `.sqlx` offline cache (they run live against a
    # freshly-migrated DB), so the compile-time macros are unavailable in test
    # seed/cleanup helpers. The gate's job is verifying production SQL; both test
    # trees are exempt.
    'crates/tests/integration/'
    'crates/tests/unit/'
    # Shared test fixtures create and drop the per-run databases themselves;
    # CREATE/DROP DATABASE cannot take a bind parameter, so the name is
    # interpolated under AssertSqlSafe. Same exemption as the two test trees.
    'crates/tests/common/'
    # The user purge walks the `user_purge_tables!` inventory: table and
    # column names come from each owning crate's registration at runtime,
    # pass SafeIdentifier and are interpolated under AssertSqlSafe. The user
    # id is always a bind.
    'crates/domain/users/src/repository/user/purge.rs'
    'crates/entry/cli/src/commands/admin/setup/'
)

runtime_hits() {
    rg -n --glob '*.rs' -e "$pattern" -- "$@" 2>/dev/null \
        | grep -Ev 'sqlx::query[a-z_]*!' || true
}

stale=""
allowlist_re=""
for entry in "${allowlist[@]}"; do
    if [[ ! -e "${entry}" ]]; then
        stale+="  ${entry} (path does not exist)"$'\n'
    elif [[ -z "$(runtime_hits "${entry}")" ]]; then
        stale+="  ${entry} (no runtime sqlx::query call left)"$'\n'
    fi
    escaped=$(printf '%s' "${entry}" | sed 's/[.[\*^$()+?{|]/\\&/g')
    if [[ "${entry}" == */ ]]; then
        allowlist_re+="${allowlist_re:+|}^${escaped}"
    else
        allowlist_re+="${allowlist_re:+|}^${escaped}:"
    fi
done

if [[ -n "${stale}" ]]; then
    echo "❌ Stale scripts/check-sqlx.sh allowlist entries — remove them:" >&2
    printf '%s' "${stale}" >&2
    exit 1
fi

hits=$(runtime_hits crates/ | grep -Ev "(${allowlist_re})" || true)

if [[ -n "${hits}" ]]; then
    echo "❌ Unverified sqlx::query calls found outside the allowlist:" >&2
    echo "${hits}" >&2
    echo "" >&2
    echo "Use sqlx::query!() / query_as!() / query_scalar!() (compile-time verified)." >&2
    echo "If the call must stay dynamic, add the path to scripts/check-sqlx.sh allowlist with justification." >&2
    exit 1
fi

echo "✅ No unverified sqlx::query calls outside the allowlist."
