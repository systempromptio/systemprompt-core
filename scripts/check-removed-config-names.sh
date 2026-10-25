#!/usr/bin/env bash
# Gate: a name the 0.73 config model removed does not come back.
#
# The catalog, sync and identity work deleted these names (credential.md
# sections 2 to 7). Only the code that reads the pre-0.73 shape may spell
# them: `config migrate`, its golden proofs and the legacy fixtures, plus
# history (CHANGELOGs and applied migrations). Any other hit is a removed
# concept growing back. Names are matched word-bounded.
#
# Allowlisted path prefixes are listed once below with the reason; a name
# is added to the list only by deleting its last live use first.
set -euo pipefail

cd "$(dirname "$0")/.."

names='surface_for|canonicalize_org_uuid|ProviderEntry|YAML_SOURCE|DASHBOARD_SOURCE|legacy_id|find_served_model|by_scope'

allowed=(
    # config migrate reads the legacy shape by definition.
    'crates/entry/cli/src/commands/admin/config/migrate/'
    # Proofs and fixtures of that migration, and the golden snapshot walker.
    'crates/tests/integration/config-migrate/'
    'crates/tests/unit/app/gateway-golden/'
    'crates/tests/common/fixtures/catalog/'
    # History.
    'CHANGELOG.md'
    '/CHANGELOG.md'
    'schema/migrations/'
    # This gate names them.
    'scripts/check-removed-config-names.sh'
)

hits=$(git ls-files -- 'crates' 'bin' 'scripts' 'justfile' ':!:*.lock' \
    | while read -r f; do
        for a in "${allowed[@]}"; do
            case "$f" in *"$a"*) continue 2 ;; esac
        done
        [ -f "$f" ] && grep -nwE "$names" "$f" 2>/dev/null | sed "s#^#$f:#"
    done || true)

if [ -n "$hits" ]; then
    echo "ERROR: check-removed-config-names: a name the 0.73 config model removed reappeared:" >&2
    echo "$hits" >&2
    echo "Delete the use, or (for legacy-shape readers) move it under config migrate." >&2
    exit 1
fi
echo "check-removed-config-names: OK"
