#!/usr/bin/env bash
# The repository is code-only (CLAUDE.md § Repository Hygiene): no tracked prose
# file outside the sanctioned set, no status/plan/report-named prose anywhere,
# and no cargo build output. A global gate: it judges every tracked path.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

prose='\.(md|txt|org|rst|adoc|pdf|docx)$'
sanctioned='^(documentation/|crates/tests/|scripts/[^/]+\.txt$|(AGENTS|CLAUDE|SECURITY)\.md$|crates/domain/agent/docs/a2aspec\.txt$)|(^|/)(README|CHANGELOG)\.md$'
hits=$( { git ls-files | grep -E "$prose" | grep -vE "$sanctioned" || true
          git ls-files | grep -iE "(status|plan|report|summary|progress|findings)[^/]*${prose}" || true
        } | sort -u)
if [ -n "$hits" ]; then
    echo "✗ prose files outside the sanctioned set (CLAUDE.md § Repository Hygiene):" >&2
    echo "$hits" | sed 's/^/    /' >&2
    exit 1
fi
echo "✓ no stray status/plan/report docs tracked"
build=$(git ls-files | grep -E '(^|/)target(-[^/]+)?/' || true)
if [ -n "$build" ]; then
    echo "✗ cargo build output tracked ($(echo "$build" | wc -l) files):" >&2
    echo "$build" | cut -d/ -f1-2 | sort -u | sed 's/^/    /' >&2
    exit 1
fi
echo "✓ no cargo build output tracked"
