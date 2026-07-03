#!/usr/bin/env bash
# Reject `let _ = <expr>.unwrap()/.expect()` in the test workspace.
#
# Discarding the result of a fallible call runs the code for its panic-on-error
# side effect but asserts nothing about what it produced — a test that can only
# fail by panicking, never by a wrong value. Bind the result and assert on it,
# or drop the `let _ =` and let the expression stand.
#
# Exemption: a line that genuinely exercises a side effect with nothing to
# assert (a coverage driver, a warm-up call) may annotate it with
# `// lint-ok: no-assert` followed by a reason.
set -uo pipefail

SEARCH_DIR="crates/tests"
PATTERN='^\s*let _ = .*\.(unwrap|expect)\('

if ! command -v rg >/dev/null 2>&1; then
    echo "check-test-value: ripgrep (rg) is required" >&2
    exit 2
fi

RAW=$(rg -n --no-heading --color=never \
    -g '*.rs' \
    "$PATTERN" "$SEARCH_DIR" 2>/dev/null || true)

# Whole-file exclusions: subprocess coverage drivers that legitimately invoke
# the CLI for its side effect and assert elsewhere (or not at all, by design).
HITS=$(printf '%s\n' "$RAW" \
    | grep -v 'lint-ok: no-assert' \
    | grep -vE '/(subprocess_smoke|subprocess_with_db|subprocess_full)\.rs:' \
    | grep -v '^[[:space:]]*$' || true)

if [ -n "$HITS" ]; then
    echo "check-test-value: \`let _ = ….unwrap()/.expect()\` in a test — panic-only, asserts nothing."
    echo "Bind the result and assert on it, or annotate a deliberate side-effect call"
    echo "with '// lint-ok: no-assert <reason>':"
    echo "$HITS"
    exit 1
fi

echo "check-test-value: OK — no unasserted discarded results in $SEARCH_DIR"
