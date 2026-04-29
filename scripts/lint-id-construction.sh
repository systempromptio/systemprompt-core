#!/usr/bin/env bash
set -uo pipefail

# Enforces typed-ID construction convention from CLAUDE.md > "Typed Identifiers":
#   - SomeId::new(s)        canonical
#   - SomeId::generate()    fresh UUID (where supported)
#   - SomeId::try_new(s)?   validated/non_empty in fallible contexts
# Banned at call sites:
#   - SomeId::from("...")
#   - "literal".into()  on known-typed-ID fields
# Macro-generated From/TryFrom impls remain on the types — this is a call-site lint.

ID_FIELDS='(user_id|agent_id|task_id|tenant_id|context_id|session_id|file_id|skill_id|client_id|artifact_id|message_id|role_id|hook_id|execution_step_id|content_id|source_id|trace_id|step_id|mcp_execution_id|ai_tool_call_id|ai_request_id|plugin_id)'

# StepId is hand-written (not define_id!); its ::new() is a zero-arg UUID generator,
# so StepId::from(s) is the canonical construction path from a known string.
PATTERN_FROM='\b(?!StepId\b)[A-Z][A-Za-z0-9_]*Id::from\('
PATTERN_INTO_LITERAL="\b${ID_FIELDS}\s*:\s*Some\(\s*\"[^\"]*\"\.into\(\)\s*\)|\b${ID_FIELDS}\s*:\s*\"[^\"]*\"\.into\(\)"

SEARCH_DIRS=(crates bin)

EXCLUDES=(
    -g '!**/target/**'
    -g '!**/.sqlx/**'
    -g '!crates/shared/identifiers/**'
    -g '!crates/tests/unit/shared/identifiers/**'
)

if ! command -v rg >/dev/null 2>&1; then
    echo "lint-id-construction: ripgrep (rg) is required" >&2
    exit 2
fi

HITS_FROM=$(rg -n --no-heading --color=never -g '*.rs' "${EXCLUDES[@]}" -e "$PATTERN_FROM" "${SEARCH_DIRS[@]}" 2>/dev/null || true)
HITS_INTO=$(rg -n --no-heading --color=never -g '*.rs' "${EXCLUDES[@]}" -e "$PATTERN_INTO_LITERAL" "${SEARCH_DIRS[@]}" 2>/dev/null || true)

ALL=""
[ -n "$HITS_FROM" ] && ALL+="$HITS_FROM"$'\n'
[ -n "$HITS_INTO" ] && ALL+="$HITS_INTO"$'\n'

if [ -z "$ALL" ]; then
    echo "lint-id-construction: OK (no banned typed-ID construction forms)"
    exit 0
fi

echo "lint-id-construction: banned typed-ID construction forms found"
echo "  use SomeId::new(x) instead of SomeId::from(x) or \"...\".into()"
echo "  see CLAUDE.md > Rust Standards > Typed Identifiers"
echo ""
printf '%s' "$ALL"
exit 1
