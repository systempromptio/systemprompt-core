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
#      - crates/app/runtime/src/builder/**  the AppContext composition root
#      - crates/entry/cli/src/**            one-shot command bodies
#      - **/jobs/**                         per-tick scheduler job bodies
#
#   2. The explicit file list below: composition-root constructors that
#      construct-and-store the repository as a field (service/router state
#      built once). Adding a file here requires justification in review —
#      the default for new code is to take the repository from AppContext
#      (`a2a_repositories()`, `content_repositories()`, `oauth_repositories()`,
#      `user_repository()`, `service_repository()`) or from the owning
#      service's stored field.
#
# The test workspace (crates/tests/**) is out of scope: fixtures construct
# repositories freely.

set -euo pipefail

cd "$(dirname "$0")/.."

ALLOWED_FILES=(
  # app: generator/scheduler context structs that construct-and-store at build
  crates/app/generator/src/prerender/context.rs
  crates/app/generator/src/rss/default_provider.rs
  crates/app/generator/src/sitemap/generator.rs
  crates/app/scheduler/src/services/job_execution.rs
  crates/app/scheduler/src/services/scheduling/mod.rs
  crates/app/scheduler/src/services/service_management.rs
  # domain: service constructors that store the repo as a field
  crates/domain/agent/src/services/a2a_server/processing/message/mod.rs
  crates/domain/agent/src/services/agent_orchestration/lifecycle/mod.rs
  crates/domain/agent/src/services/agent_orchestration/monitor.rs
  crates/domain/agent/src/services/agent_orchestration/orchestrator/mod.rs
  crates/domain/agent/src/services/agent_orchestration/reconciler.rs
  crates/domain/agent/src/services/artifact_publishing.rs
  crates/domain/agent/src/services/context_provider.rs
  crates/domain/ai/src/services/core/ai_service/service.rs
  crates/domain/ai/src/services/core/image_service.rs
  crates/domain/ai/src/services/gateway/ingestion.rs
  crates/domain/analytics/src/services/ai_provider.rs
  crates/domain/analytics/src/services/service.rs
  crates/domain/analytics/src/services/session_cleanup.rs
  crates/domain/content/src/services/content_provider.rs
  crates/domain/content/src/services/ingestion/mod.rs
  crates/domain/content/src/services/link/analytics.rs
  crates/domain/content/src/services/link/generation.rs
  crates/domain/content/src/services/search/mod.rs
  crates/domain/evaluation/src/services/evaluation_service.rs
  crates/domain/files/src/services/ai_provider.rs
  crates/domain/files/src/services/upload/service.rs
  crates/domain/mcp/src/middleware/session_handler/mod.rs
  crates/domain/mcp/src/middleware/session_handler/session_store.rs
  crates/domain/mcp/src/orchestration/state.rs
  crates/domain/mcp/src/services/database/mod.rs
  crates/domain/mcp/src/services/monitoring/proxy_health.rs
  crates/domain/oauth/src/services/cimd/validator.rs
  crates/domain/oauth/src/state.rs
  crates/domain/users/src/services/api_key_service.rs
  crates/domain/users/src/services/device_cert_service.rs
  crates/domain/users/src/services/user/mod.rs
  # entry/api: router-scoped state and service structs built once
  crates/entry/api/src/routes/analytics/mod.rs
  crates/entry/api/src/routes/engagement/mod.rs
  crates/entry/api/src/routes/proxy/mcp/mod.rs
  crates/entry/api/src/services/gateway/policy.rs
  crates/entry/api/src/services/gateway/repositories.rs
  crates/entry/api/src/services/health/monitor.rs
  crates/entry/api/src/services/middleware/analytics/mod.rs
  crates/entry/api/src/services/middleware/jwt/revocation.rs
  crates/entry/api/src/services/proxy/audit/mod.rs
  crates/entry/api/src/services/server/routes/mod.rs
  # infra: outbox/logging services that construct-and-store
  crates/infra/events/src/services/bridge.rs
  crates/infra/events/src/services/routing.rs
  crates/infra/logging/src/services/database_log.rs
  crates/infra/logging/src/services/maintenance.rs
  crates/infra/logging/src/services/retention/scheduler.rs
)

declare -A allowed
for f in "${ALLOWED_FILES[@]}"; do
  allowed["$f"]=1
done

fail=0
while IFS= read -r file; do
  case "$file" in
    crates/tests/*) continue ;;
    crates/*/*/src/repository/*) continue ;;
    crates/app/runtime/src/builder/*) continue ;;
    crates/entry/cli/src/*) continue ;;
    */jobs/*) continue ;;
  esac
  if ! grep -q '\b[A-Z][A-Za-z0-9_]*Repository::new(' "$file"; then
    continue
  fi
  if [[ -z "${allowed[$file]:-}" ]]; then
    echo "lint-repo-construction: ad-hoc repository construction in $file"
    grep -n '\b[A-Z][A-Za-z0-9_]*Repository::new(' "$file" | sed 's/^/  /'
    fail=1
  fi
done < <(git ls-files 'crates/*/*/src/**/*.rs')

if [[ "$fail" -ne 0 ]]; then
  echo
  echo "Repositories are constructed once at a composition root (AppContext"
  echo "accessors, router state, or an owning service's stored field) and"
  echo "injected. See CLAUDE.md 'Rust Standards' and scripts/lint-repo-construction.sh."
  exit 1
fi

echo "lint-repo-construction: OK"
