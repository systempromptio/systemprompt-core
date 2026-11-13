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

#   scripts/lint-repo-construction.sh                        # core roots + allowlist
#   scripts/lint-repo-construction.sh --paths P...           # only files under P
#   scripts/lint-repo-construction.sh [--allowlist FILE] ROOT...
#
# Without roots the script runs from the repository root; with roots, paths
# (and allowlist entries) resolve against the caller's working directory. A
# directory literally named `tests` is never scanned. With `--paths`, stale
# allowlist entries are not judged.

set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"

ALLOWLIST=""
ROOTS=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --allowlist) ALLOWLIST="$2"; shift 2 ;;
        --allowlist=*) ALLOWLIST="${1#--allowlist=}"; shift ;;
        -*) echo "usage: $0 [--allowlist FILE] [ROOT...]" >&2; exit 2 ;;
        *) ROOTS+=("$1"); shift ;;
    esac
done
gate_require_one_scope "${#ROOTS[@]}"
if [ "${#ROOTS[@]}" -eq 0 ]; then
    cd "$GATE_ROOT"
    ALLOWLIST="${ALLOWLIST:-scripts/repo-construction-allowlist.txt}"
fi

ALLOWED_FILES=()
if [ -n "$ALLOWLIST" ]; then
    mapfile -t ALLOWED_FILES < <(gate_allowlist "$ALLOWLIST")
fi

list_sources() {
    if [ "${#ROOTS[@]}" -eq 0 ]; then
        gate_files 'crates/*/*/src/**/*.rs'
    else
        gate_tree_files "${ROOTS[@]}" | grep -E '\.rs$' || true
    fi
}

declare -A allowed
for f in "${ALLOWED_FILES[@]}"; do
  allowed["$f"]=1
done

pattern='\b[A-Z][A-Za-z0-9_]*Repository::new('

fail=0
declare -A seen
while IFS= read -r file; do
  case "$file" in
    */tests/*|tests/*) continue ;;
    */src/repository/*) continue ;;
    */runtime/src/builder/*) continue ;;
    */cli/src/*) continue ;;
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
done < <(list_sources)

for f in "${ALLOWED_FILES[@]}"; do
  if ! gate_full_mode; then
    break
  elif [[ ! -f "$f" ]]; then
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
