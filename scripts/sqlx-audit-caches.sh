#!/usr/bin/env bash
# Every per-crate `.sqlx/` cache must hold exactly the queries its own `src/`
# issues. `cargo sqlx prepare` emits whatever the macro expansion touched in
# that run, so a dependency re-expanded alongside the crate leaks its queries
# into the crate's cache (0.51.0: `domain/ai` shipped scheduler queries).
#
# Ownership is decided by SQL text: the `query!` family only accepts a string
# literal, so a cache entry is owned iff its whitespace-normalised `.query`
# appears, whitespace-normalised, somewhere in the crate's `src/**/*.rs`.
#
#   scripts/sqlx-audit-caches.sh            # report foreign entries, exit 1 on any
#   scripts/sqlx-audit-caches.sh --prune    # delete them and list what went
#   scripts/sqlx-audit-caches.sh [--prune] <crate-dir>...   # only these crates
#   scripts/sqlx-audit-caches.sh --cache-root DIR ...       # audit DIR/<crate>/.sqlx
#                                                           # against <crate>/src
set -euo pipefail
shopt -s nullglob

PRUNE=0
CACHE_ROOT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --prune) PRUNE=1; shift ;;
        --cache-root) CACHE_ROOT="$2"; shift 2 ;;
        *) break ;;
    esac
done

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

if [ $# -gt 0 ]; then
    CRATES=("$@")
else
    mapfile -t CRATES < <(find crates -mindepth 2 -maxdepth 3 -type d -name .sqlx -not -path 'crates/tests/*' | xargs -n1 dirname | sort)
fi

total_foreign=0
for dir in "${CRATES[@]}"; do
    cache="${CACHE_ROOT:+$CACHE_ROOT/}$dir/.sqlx"
    [ -d "$cache" ] || continue
    foreign=$(python3 - "$dir" "$cache" <<'PY'
import glob, json, os, re, sys

crate, cache = sys.argv[1], sys.argv[2]

def norm(s):
    return re.sub(r"\s+", " ", s).strip()

# A `\` at the end of a line inside a Rust string literal swallows the newline
# and the next line's leading whitespace; the SQL sqlx recorded has nothing
# there, so drop the sequence before normalising.
corpus = []
for path in glob.glob(os.path.join(crate, "src", "**", "*.rs"), recursive=True):
    with open(path, encoding="utf-8", errors="ignore") as fh:
        text = fh.read()
    text = re.sub(r"\\\n\s*", "", text)
    corpus.append(norm(text))
corpus = "\n".join(corpus)

for entry in sorted(glob.glob(os.path.join(cache, "query-*.json"))):
    with open(entry, encoding="utf-8") as fh:
        query = json.load(fh).get("query", "")
    if norm(query) not in corpus:
        print(f"{entry}\t{norm(query)[:90]}")
PY
)
    [ -n "$foreign" ] || continue
    count=$(printf '%s\n' "$foreign" | wc -l)
    total_foreign=$((total_foreign + count))
    if [ $PRUNE -eq 1 ]; then
        echo "$cache: pruning $count foreign entr(y/ies) not issued by $dir/src:" >&2
    else
        echo "$cache: $count foreign entr(y/ies) not issued by $dir/src:" >&2
    fi
    while IFS=$'\t' read -r file sql; do
        printf '  %s\n    %s\n' "$(basename "$file")" "$sql" >&2
        if [ $PRUNE -eq 1 ]; then
            rm -f "$file"
        fi
    done <<<"$foreign"
done

if [ $total_foreign -gt 0 ]; then
    if [ $PRUNE -eq 1 ]; then
        echo "pruned $total_foreign foreign cache entr(y/ies)" >&2
        exit 0
    fi
    echo "error: $total_foreign cache entr(y/ies) belong to another crate. Run 'just sqlx-prepare-publish' (which prunes them) or 'scripts/sqlx-audit-caches.sh --prune'." >&2
    exit 1
fi
echo "✓ every per-crate .sqlx cache holds only its own queries (${#CRATES[@]} crates)"
