#!/usr/bin/env bash
# Pre-merge gate: repositories are constructed at composition roots and
# injected; ad-hoc `SomeRepository::new(&pool)` inside handler/method bodies is
# the anti-pattern this gate blocks (it made the 0.29.0 TaskRepository
# decoupling cascade through 22 call sites).
#
# A `*Repository::new(` call in the main workspace is legal only when the file
# falls in one of two tiers:
#
#   1. Pattern-exempt (structural):
#      - crates/*/*/src/repository/**       bundles and sub-repo construction
#        (this includes crates/entry/api/src/repository/**, the entry-layer
#        composition modules for router state)
#      - crates/app/runtime/src/builder/**  the AppContext composition root
#      - crates/entry/cli/src/**            one-shot command bodies
#      - **/jobs/**                         per-tick scheduler job bodies
#
#   2. The explicit file list below. Every entry is structurally unable to use
#      an AppContext accessor and carries its reason. Adding a file requires
#      justification in review — the default for new code is to take the
#      repository from AppContext (`a2a_repositories()`,
#      `content_repositories()`, `oauth_repositories()`, `ai_repositories()`,
#      `analytics_repositories()`, `user_repository()`, `service_repository()`,
#      `file_repository()`, `mcp_session_repository()`) or from the owning
#      service's stored field.
#
# Bundle constructors (`*Repositories::new(`) never match the regex — the
# plural suffix is the sanctioned composition form, not a gap to close.
#
# The allowlist is checked for staleness: an entry whose file is gone or no
# longer constructs a repository fails the gate, so dead entries cannot
# accumulate.
#
# The test workspace (crates/tests/**) is out of scope: fixtures construct
# repositories freely.

set -euo pipefail

cd "$(dirname "$0")/.."

ALLOWED_FILES=(
  # infra sits below app/runtime and cannot name AppContext; each of these is
  # the composition root for its own repository.
  crates/infra/events/src/services/bridge.rs
  crates/infra/events/src/services/routing.rs
  crates/infra/logging/src/services/database_log.rs
  crates/infra/logging/src/services/maintenance.rs
  crates/infra/logging/src/services/retention/scheduler.rs
  # scheduler-owned repos (SchedulerRepository, JobRepository,
  # LoggingRepository) have exactly one consuming crate; an AppContext
  # accessor for them is not warranted.
  crates/app/scheduler/src/services/job_execution.rs
  crates/app/scheduler/src/services/scheduling/mod.rs
  # the logging AnalyticsRepository is an infra repo with no AppContext
  # accessor; the middleware constructs-and-stores it once at server build.
  crates/entry/api/src/services/middleware/analytics/mod.rs
)

declare -A allowed
for f in "${ALLOWED_FILES[@]}"; do
  allowed["$f"]=1
done

pattern='\b[A-Z][A-Za-z0-9_]*Repository::new('

fail=0
declare -A seen
while IFS= read -r file; do
  case "$file" in
    crates/tests/*) continue ;;
    crates/*/*/src/repository/*) continue ;;
    crates/app/runtime/src/builder/*) continue ;;
    crates/entry/cli/src/*) continue ;;
    */jobs/*) continue ;;
  esac
  if ! grep -q "$pattern" "$file"; then
    continue
  fi
  if [[ -n "${allowed[$file]:-}" ]]; then
    seen["$file"]=1
  else
    echo "lint-repo-construction: ad-hoc repository construction in $file"
    grep -n "$pattern" "$file" | sed 's/^/  /'
    fail=1
  fi
done < <(git ls-files 'crates/*/*/src/**/*.rs')

for f in "${ALLOWED_FILES[@]}"; do
  if [[ ! -f "$f" ]]; then
    echo "lint-repo-construction: stale allowlist entry (file missing): $f"
    fail=1
  elif [[ -z "${seen[$f]:-}" ]]; then
    echo "lint-repo-construction: stale allowlist entry (no repository construction): $f"
    fail=1
  fi
done

if [[ "$fail" -ne 0 ]]; then
  echo
  echo "Repositories are constructed once at a composition root (AppContext"
  echo "accessors, router state, or an owning service's stored field) and"
  echo "injected. See CLAUDE.md 'Rust Standards' and scripts/lint-repo-construction.sh."
  exit 1
fi

echo "lint-repo-construction: OK"
