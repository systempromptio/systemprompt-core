#!/usr/bin/env bash
# `UserId::admin()` is an untyped actor: it names no principal, so an audit row
# or authorization decision built on it cannot say who acted. It is spelled only
# at the sanctioned bootstrap and system call sites listed below; everywhere
# else the caller's real UserId is passed through.
#
#   scripts/lint-no-untyped-admin.sh [--paths P...]
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
cd "$GATE_ROOT"

sanctioned=(
    crates/shared/identifiers/src/actor.rs
    crates/shared/identifiers/src/bootstrap.rs
    crates/shared/identifiers/src/user.rs
    crates/entry/cli/src/commands/admin/bootstrap.rs
    crates/entry/cli/src/commands/infrastructure/jobs/run.rs
    crates/shared/manifest/src/services/scheduler.rs
    crates/domain/mcp/src/services/registry/manager.rs
    crates/infra/logging/src/models/log_entry.rs
)

hits=$(grep -rn 'UserId::admin()' crates/ --include='*.rs' \
    | grep -v 'crates/tests/' \
    | grep -vF -f <(printf '%s\n' "${sanctioned[@]}") \
    | gate_filter \
    || true)
if [ -n "$hits" ]; then
    echo "lint-no-untyped-admin: untyped UserId::admin() outside the sanctioned call sites:"
    echo "$hits"
    exit 1
fi
