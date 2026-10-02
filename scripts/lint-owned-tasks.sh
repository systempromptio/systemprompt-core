#!/usr/bin/env bash
# Every task and thread has an owner that joins, drains or aborts it
# (CLAUDE.md § Rust Standards: owned background tasks).
#
# Over the production crates, the facade and the bridge:
#
#   detached-spawn    `tokio::spawn(`, `tokio::task::spawn(`, `task::spawn(`,
#                     `spawn_local(`, `std::thread::spawn(` or
#                     `thread::spawn(` outside an owner module. Spawn through
#                     `systemprompt_traits::BackgroundTasks` (one-shot or
#                     cancellable work drained at shutdown) or
#                     `systemprompt_traits::OwnedTask` (a single task whose
#                     owner joins or aborts it); the bridge spawns through
#                     its own owners in bin/bridge/src/tasks.rs. Scoped
#                     threads (`std::thread::scope`) are owned by
#                     construction and are not matched.
#
# Reported, not gated (grep cannot see through helper fns):
#
#   blocking-in-async a `std::process::Command` `.output()`/`.status()`/
#                     `.wait()` line inside an `async fn` body that is not
#                     itself awaited (tokio's Command) and not inside a
#                     `spawn_blocking` closure. Printed per crate with
#                     --report.
#
#   scripts/lint-owned-tasks.sh            # gate
#   scripts/lint-owned-tasks.sh --count    # detached-spawn hit count only
#   scripts/lint-owned-tasks.sh --report   # per-crate counts of both rules
set -uo pipefail
cd "$(dirname "$0")/.."
command -v rg >/dev/null || { echo "lint-owned-tasks: ripgrep required" >&2; exit 2; }

MODE=gate
case "${1:-}" in
    --count) MODE=count ;;
    --report) MODE=report ;;
    "") ;;
    *) echo "usage: $0 [--count|--report]" >&2; exit 2 ;;
esac

ROOTS=(crates systemprompt/src bin/bridge/src)
GLOBS=(-g '*.rs' -g '!crates/tests/**' -g '!**/target/**' -g '!**/build.rs')

# The owner modules: the only places a raw spawn may appear.
OWNERS=(
    crates/shared/traits/src/background_tasks.rs
    bin/bridge/src/tasks.rs
)

owner_filter() {
    local pattern
    pattern=$(printf '%s\n' "${OWNERS[@]}" | sed 's/[.]/\\./g' | paste -sd'|' -)
    grep -Ev "^(${pattern}):"
}

spawn_hits=$(rg -n --no-heading --color=never "${GLOBS[@]}" \
    -e '(^|[^A-Za-z0-9_:])(tokio::spawn|tokio::task::spawn|task::spawn|std::thread::spawn|thread::spawn)[[:space:]]*\(' \
    -e '(^|[^A-Za-z0-9_])spawn_local[[:space:]]*\(' \
    "${ROOTS[@]}" \
    | grep -Ev '^[^:]+:[0-9]+:[[:space:]]*//' \
    | owner_filter)

scanned=$(rg -l --no-messages "${GLOBS[@]}" -e 'fn ' "${ROOTS[@]}" | wc -l | tr -d ' ')
[ "$scanned" -gt 0 ] || { echo "lint-owned-tasks: no Rust sources found — scope broken?" >&2; exit 1; }

crate_of() {
    awk -F: '{
        split($1, p, "/")
        if (p[1] == "crates") print p[1] "/" p[2] "/" p[3]
        else if (p[1] == "bin") print p[1] "/" p[2]
        else print p[1]
    }'
}

blocking_hits() {
    rg -l --no-messages "${GLOBS[@]}" -e 'std::process::Command|use std::process::\{?[^;]*Command' "${ROOTS[@]}" \
    | while IFS= read -r file; do
        awk -v f="$file" '
            function count(s, c,   n, i) { n = 0; for (i = 1; i <= length(s); i++) if (substr(s, i, 1) == c) n++; return n }
            /^[[:space:]]*\/\// { next }
            {
                line = $0
                if (!in_async && line ~ /(^|[^A-Za-z0-9_])async[[:space:]]+(unsafe[[:space:]]+)?fn[[:space:]]/ && line !~ /;[[:space:]]*$/) {
                    in_async = 1; depth = 0; opened = 0
                }
                if (in_async) {
                    if (!in_blocking && line ~ /spawn_blocking[[:space:]]*\(/) { in_blocking = 1; bdepth = depth }
                    if (!in_blocking && line ~ /\.(output|status|wait)\(\)/ && line !~ /\.await/) print f ":" NR ": " line
                    depth += count(line, "{") - count(line, "}")
                    if (count(line, "{") > 0) opened = 1
                    if (in_blocking && depth <= bdepth && line ~ /\)/) in_blocking = 0
                    if (opened && depth <= 0) { in_async = 0; in_blocking = 0 }
                }
            }
        ' "$file"
    done
}

if [ "$MODE" = count ]; then
    printf '%s' "$spawn_hits" | grep -c . ; exit 0
fi

if [ "$MODE" = report ]; then
    echo "detached-spawn per crate:"
    printf '%s\n' "$spawn_hits" | grep . | crate_of | sort | uniq -c | sort -rn
    echo "blocking-in-async per crate (reported, not gated):"
    blocking_hits | crate_of | sort | uniq -c | sort -rn
    exit 0
fi

if [ -n "$spawn_hits" ]; then
    echo "lint-owned-tasks: spawn through BackgroundTasks / OwnedTask (bridge: bin/bridge/src/tasks.rs), never detached:" >&2
    printf '%s\n' "$spawn_hits" | sed 's/^/  detached-spawn: /' >&2
    echo "per crate:" >&2
    printf '%s\n' "$spawn_hits" | crate_of | sort | uniq -c | sort -rn >&2
    exit 1
fi
echo "lint-owned-tasks: OK ($scanned files, owners: ${#OWNERS[@]})"
