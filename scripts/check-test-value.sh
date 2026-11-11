#!/usr/bin/env bash
# Reject test code that can only fail by panicking, or can never fail.
#
# Usage: check-test-value.sh [DIR]   (default: crates/tests)
#
# Every rule matches across line breaks, so a rustfmt-wrapped expression is
# caught exactly like a one-line one. A hit is reported at the line where the
# offending statement starts.
#
# Exemption: a statement that genuinely exercises a side effect with nothing
# to assert (a coverage driver, a warm-up call) may carry
# `// lint-ok: no-assert` followed by a reason on the line it starts.
set -uo pipefail

SEARCH_DIR="${1:-crates/tests}"

if ! command -v rg >/dev/null 2>&1; then
    echo "check-test-value: ripgrep (rg) is required" >&2
    exit 2
fi

# Rule 1: `let _ = <expr>.unwrap()/.expect()` — panic-only, asserts nothing.
# `[^;]` spans newlines under -U, so the statement may be wrapped.
PATTERN_PANIC='(?m)^[ \t]*let _ =[^;]*?\.(unwrap|expect)\('

# Rule 2: `let _ = <expr>.len()/.count();` — discards a query/collection size
# for no observable effect. On a scoped query, bind it and assert seeded-row
# membership, otherwise a per-row property.
PATTERN_SIZE='(?m)^[ \t]*let _ =\s*[A-Za-z_][A-Za-z0-9_.\s]*\.(len|count)\(\)\s*;'

# Rule 3: a wiremock JSON-RPC mount that matches on `method` alone. The
# request body is the wire contract; match the whole frame (`jsonrpc`, `id`,
# `method`, `params`).
PATTERN_METHOD_ONLY='body_partial_json\(\s*serde_json::json!\(\{\s*"method":\s*"[^"]+"\s*\}\)'

# Rule 4: an assertion that holds for every value — `assert!(true)`, a
# disjunction of a predicate and its complement (`is_ok() || is_err()`,
# `is_some() || is_none()`), or a count, length or total compared `>= 0`.
PATTERN_TAUTOLOGY='assert!\(\s*true\s*[,)]|\.is_ok\(\)\s*\|\|\s*[^;|]*?\.is_err\(\)|\.is_err\(\)\s*\|\|\s*[^;|]*?\.is_ok\(\)|\.is_some\(\)\s*\|\|\s*[^;|]*?\.is_none\(\)|\.is_none\(\)\s*\|\|\s*[^;|]*?\.is_some\(\)|assert!\(\s*[^;]*?(count|len|total)[^;]*?>=\s*0\s*[,)]'

# Rule 5: `assert_eq!`/`assert_ne!` comparing an expression with itself.
PATTERN_SELF_EQ='assert_(eq|ne)!\(\s*([^,()]+?)\s*,\s*\2\s*[,)]'

# Report the first line of each multi-line match. `--json` gives each match's
# start line, so a wrapped statement is attributed to the line it starts on.
first_lines() {
    local pattern="$1"
    rg -U --json -g '*.rs' "$pattern" "$SEARCH_DIR" 2>/dev/null \
        | python3 -c '
import json, sys
for raw in sys.stdin:
    event = json.loads(raw)
    if event.get("type") != "match":
        continue
    data = event["data"]
    text = data["lines"].get("text", "")
    first = text.split("\n", 1)[0]
    path = data["path"]["text"]
    print("%s:%d:%s" % (path, data["line_number"], first))
'
}

# Whole-file exclusions: subprocess coverage drivers that legitimately invoke
# the CLI for its side effect and assert elsewhere (or not at all, by design).
filter_hits() {
    grep -v 'lint-ok: no-assert' \
        | grep -vE '/(subprocess_smoke|subprocess_with_db|subprocess_full)\.rs:' \
        | grep -v '^[[:space:]]*$' || true
}

STATUS=0

report() {
    local hits="$1"
    shift
    if [ -n "$hits" ]; then
        printf '%s\n' "$@"
        printf '%s\n' "$hits"
        STATUS=1
    fi
}

report "$(first_lines "$PATTERN_PANIC" | filter_hits)" \
    "check-test-value: \`let _ = ….unwrap()/.expect()\` in a test — panic-only, asserts nothing." \
    "Bind the result and assert on it, or annotate a deliberate side-effect call" \
    "with '// lint-ok: no-assert <reason>':"

report "$(first_lines "$PATTERN_SIZE" | filter_hits)" \
    "check-test-value: \`let _ = ….len()/.count();\` in a test — discards a size, asserts nothing." \
    "Bind the value and assert a real contract (seeded-row membership on a scoped" \
    "query, or a per-row property), or annotate with '// lint-ok: no-assert <reason>':"

report "$(first_lines "$PATTERN_METHOD_ONLY" | filter_hits)" \
    "check-test-value: a JSON-RPC mount that matches on \`method\` alone — the frame is the contract." \
    "Match the whole request (jsonrpc, id, method, params):"

report "$(first_lines "$PATTERN_TAUTOLOGY" | filter_hits)" \
    "check-test-value: an assertion that no value can fail (\`assert!(true)\`, \`a || !a\`, \`count >= 0\`)." \
    "Assert the value the code under test must produce:"

SELF_EQ=$(grep -rnP --include='*.rs' "$PATTERN_SELF_EQ" "$SEARCH_DIR" 2>/dev/null | filter_hits)
report "$SELF_EQ" \
    "check-test-value: an assertion comparing an expression with itself." \
    "Compare the produced value with an independent expected value:"

if [ "$STATUS" -ne 0 ]; then
    exit 1
fi

echo "check-test-value: OK — no unasserted or tautological assertions in $SEARCH_DIR"
