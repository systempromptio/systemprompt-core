#!/usr/bin/env bash
# Pre-merge gate enforcing the declarative-schema / imperative-migration split.
#
# Scans every crates/**/schema/*.sql (not under schema/migrations/) for the
# imperative-SQL forms the install-time linter rejects. The runner's
# schema_linter is the authoritative enforcement point at boot — this script
# is a fast pre-merge preview so authors hit the rule on their workstation,
# not on a customer's server at 3am.
#
# A SQL line counts as a violation if its first non-whitespace token is one of:
#   ALTER, DROP, UPDATE, INSERT, DELETE, TRUNCATE, GRANT, REVOKE, DO
# Exception: DROP {VIEW|MATERIALIZED VIEW|INDEX|TRIGGER} IF EXISTS is allowed —
# these are stateless derived objects, dropped only to be recreated by the
# sibling CREATE statement (matches the install-time linter's carve-out).
# Lines inside a dollar-quoted body (`$$ … $$`, the plpgsql function form the
# allowlist admits) are skipped: a DELETE inside a retention function is the
# function's behaviour, not a schema mutation. The install-time linter does the
# precise check.

set -euo pipefail

ROOT="${1:-crates}"

if ! command -v grep >/dev/null 2>&1; then
    echo "lint-schema: grep not found" >&2
    exit 2
fi

# shellcheck disable=SC2207
files=($(find "$ROOT" -type f -name '*.sql' \
    -path '*/schema/*' \
    -not -path '*/schema/migrations/*' \
    -not -path '*/target/*' \
    | sort))

if [ ${#files[@]} -eq 0 ]; then
    echo "lint-schema: no schema files under $ROOT"
    exit 0
fi

violations=0
forbidden='^[[:space:]]*(ALTER|DROP|UPDATE|INSERT|DELETE|TRUNCATE|GRANT|REVOKE|DO)\b'
safe_drop='^[[:space:]]*DROP[[:space:]]+(MATERIALIZED[[:space:]]+VIEW|VIEW|INDEX|TRIGGER)[[:space:]]+IF[[:space:]]+EXISTS\b'

# Prints the file with every line inside a `$$ … $$` body blanked, so the
# line numbers reported below still point at the original file.
outside_dollar_bodies() {
    awk '{
        line = $0
        out = ""
        while (line != "") {
            i = index(line, "$$")
            if (i == 0) { if (!inside) out = out line; break }
            piece = substr(line, 1, i - 1)
            if (!inside) out = out piece
            inside = !inside
            line = substr(line, i + 2)
        }
        print out
    }' "$1"
}

for f in "${files[@]}"; do
    if matches=$(outside_dollar_bodies "$f" | grep -nEi "$forbidden" || true); [ -n "$matches" ]; then
        file_violations=0
        while IFS= read -r line; do
            content="${line#*:}"
            if echo "$content" | grep -qEi "$safe_drop"; then
                continue
            fi
            echo "$f:$line: imperative SQL in declarative schema — move to schema/migrations/NNN_<name>.sql"
            file_violations=$((file_violations + 1))
        done <<< "$matches"
        if [ $file_violations -gt 0 ]; then
            violations=$((violations + 1))
        fi
    fi
done

if [ $violations -gt 0 ]; then
    echo "" >&2
    echo "lint-schema: $violations file(s) contain imperative SQL." >&2
    echo "Allowed in schema/: CREATE TABLE IF NOT EXISTS, CREATE INDEX IF NOT EXISTS," >&2
    echo "                    CREATE [OR REPLACE] FUNCTION/VIEW/TRIGGER, CREATE TYPE," >&2
    echo "                    CREATE EXTENSION IF NOT EXISTS, COMMENT ON." >&2
    echo "All other statements must live in schema/migrations/NNN_<name>.sql and be" >&2
    echo "declared via Extension::migrations()." >&2
    exit 1
fi

echo "lint-schema: ${#files[@]} schema file(s) checked, all declarative."
