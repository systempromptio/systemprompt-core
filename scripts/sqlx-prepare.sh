#!/usr/bin/env bash
# Regenerate SQLx offline caches deterministically.
#
#   scripts/sqlx-prepare.sh workspace   # the root cache (development only, gitignored)
#   scripts/sqlx-prepare.sh publish     # every published crate's own .sqlx/ (committed)
#
# `cargo sqlx prepare` only emits query data for crates it re-expands in that
# run and prunes whatever it did not emit, so its output depends on target/
# state unless every relevant package is cleaned first. It also reads
# DATABASE_URL from the shell, not from cargo's `[env]` table, which is where
# this repository keeps it. Both are handled here, and every cache is
# snapshotted and restored if the run fails, so a half-run never leaves a
# pruned cache behind. A cache that ends smaller than it started is rejected
# unless PREPARE_ALLOW_PRUNE=1 — removing a query is a deliberate act.
set -euo pipefail
shopt -s nullglob

MODE="${1:-}"
case "$MODE" in workspace|publish) ;; *)
    echo "usage: $0 workspace|publish" >&2; exit 2 ;;
esac

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

if [ -z "${DATABASE_URL:-}" ]; then
    DATABASE_URL=$(sed -n 's/^DATABASE_URL *= *"\(.*\)"/\1/p' .cargo/config.toml | head -1)
fi
if [ -z "${DATABASE_URL:-}" ]; then
    echo "error: DATABASE_URL is not set and .cargo/config.toml carries none" >&2
    exit 1
fi
export DATABASE_URL
export SQLX_OFFLINE=false
if command -v pg_isready >/dev/null 2>&1 && ! pg_isready -d "$DATABASE_URL" -t 2 >/dev/null 2>&1; then
    echo "error: database not reachable at $DATABASE_URL" >&2
    exit 1
fi

# Every published crate under crates/ that depends on sqlx. `entry/api` issues
# no SQL through the query macros and so needs no cache of its own; anything
# else that appears here and is not prepared is a publishing defect.
mapfile -t SQLX_CRATES < <(
    cargo metadata --no-deps --format-version 1 \
        | jq -r --arg root "$ROOT/" '.packages[]
            | select(.dependencies[]?.name == "sqlx")
            | "\(.name)\t\(.manifest_path | sub($root; "") | sub("/Cargo.toml$"; ""))"' \
        | grep -P '\tcrates/' | grep -Pv '\tcrates/(tests|entry/api)' | sort
)
# Only crates that invoke a query macro own cache entries; a crate that merely
# derives `sqlx::Type` would otherwise gain an empty or dependency-only cache.
MACRO_RE='(sqlx::)?query(_as|_scalar|_file|_file_as|_file_scalar|_with)?!'
filtered=()
for entry in "${SQLX_CRATES[@]}"; do
    # Counted rather than `grep -q`: under `pipefail` an early-exiting reader
    # SIGPIPEs the producer and the test fails at random.
    hits=$(grep -rhE "$MACRO_RE" "${entry#*	}/src" --include='*.rs' 2>/dev/null | grep -vcE '^[[:space:]]*//' || true)
    if [ "${hits:-0}" -gt 0 ]; then
        filtered+=("$entry")
    fi
done
SQLX_CRATES=("${filtered[@]}")
if [ ${#SQLX_CRATES[@]} -eq 0 ]; then
    echo "error: no crate invokes a sqlx query macro; nothing to prepare" >&2
    exit 1
fi

cache_dirs() {
    if [ "$MODE" = workspace ]; then
        echo .sqlx
    else
        for entry in "${SQLX_CRATES[@]}"; do
            echo "${entry#*	}/.sqlx"
        done
    fi
}

SNAP=$(mktemp -d)
for d in $(cache_dirs); do
    if [ -d "$d" ]; then
        mkdir -p "$SNAP/$d"
        cp "$d"/*.json "$SNAP/$d/" 2>/dev/null || true
    fi
done
restore() {
    for d in $(cache_dirs); do
        if [ -d "$SNAP/$d" ]; then
            rm -rf "$ROOT/$d"
            mkdir -p "$ROOT/$d"
            cp "$SNAP/$d"/*.json "$ROOT/$d/" 2>/dev/null || true
        fi
    done
}
cleanup() {
    status=$?
    if [ $status -ne 0 ]; then
        echo "prepare failed (exit $status); restoring every .sqlx cache to its previous state" >&2
        restore
    fi
    rm -rf "$SNAP"
    exit $status
}
trap cleanup EXIT

if [ "$MODE" = workspace ]; then
    for entry in "${SQLX_CRATES[@]}"; do
        cargo clean -p "${entry%%	*}" 2>/dev/null || true
    done
    echo "Preparing workspace cache..."
    cargo sqlx prepare --workspace
else
    # A crate's own cache must hold exactly its own queries. A dependency
    # re-expanded during this crate's check would leak its queries in, and
    # which dependency that is depends on the order of the loop — so every
    # dependency is made fresh up front and only the crate itself is cleaned.
    echo "Checking the workspace so dependencies are fresh..."
    cargo check --workspace --all-features
    for entry in "${SQLX_CRATES[@]}"; do
        name="${entry%%	*}"
        dir="${entry#*	}"
        echo "Preparing $dir..."
        cargo clean -p "$name" 2>/dev/null || true
        (cd "$dir" && cargo sqlx prepare)
    done
fi

pruned=0
for d in $(cache_dirs); do
    [ -d "$SNAP/$d" ] || continue
    for f in "$SNAP/$d"/*.json; do
        if [ ! -f "$d/$(basename "$f")" ]; then
            if [ $pruned -eq 0 ]; then
                echo "queries removed from the cache:" >&2
            fi
            pruned=$((pruned + 1))
            printf '  %s: %s\n' "$d" "$(jq -r '.query' "$f" | tr -s ' \n' ' ' | cut -c1-100)" >&2
        fi
    done
done
if [ $pruned -gt 0 ] && [ "${PREPARE_ALLOW_PRUNE:-0}" != "1" ]; then
    echo "error: $pruned cached quer(y/ies) disappeared. If the SQL was really deleted, re-run with PREPARE_ALLOW_PRUNE=1." >&2
    exit 1
fi

if [ "$MODE" = publish ]; then
    echo "Per-crate caches prepared. Commit them before publishing: git add crates/*/*/.sqlx"
else
    echo "Workspace cache prepared ($(ls .sqlx | wc -l) queries)"
fi
