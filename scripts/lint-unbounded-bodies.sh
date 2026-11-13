#!/usr/bin/env bash
# Request and response bodies are read with a bound (CLAUDE.md § Rust
# Standards). `DefaultBodyLimit` is honoured only by the `Json`/`Bytes`
# extractors; a raw body read bypasses it, so an unbounded one lets a single
# caller pin arbitrary memory.
#
# Over the production crates, the facade and the bridge:
#
#   unbounded-to-bytes   `to_bytes(<expr>, usize::MAX)`: an axum body read
#                        with no limit. Use
#                        `systemprompt_api::services::middleware::read_body_bounded`
#                        (or pass a named limit) instead.
#   unbounded-collect    `body.collect().await` / `into_body().collect().await`
#                        (`BodyExt::collect`), which buffers the whole body
#                        with no limit. Wrap the body in
#                        `http_body_util::Limited` first, or use
#                        `read_body_bounded`.
#
#   scripts/lint-unbounded-bodies.sh            # gate over the core roots
#   scripts/lint-unbounded-bodies.sh --paths P...  # only files under P
#   scripts/lint-unbounded-bodies.sh ROOT...    # scan other roots
#
# A directory literally named `tests` or `target` is never scanned.
set -uo pipefail
command -v rg >/dev/null || { echo "lint-unbounded-bodies: ripgrep required" >&2; exit 2; }

. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
gate_require_one_scope "$#"
if [ "$#" -eq 0 ]; then
    cd "$GATE_ROOT" || exit 2
    mapfile -t ROOTS < <(gate_roots crates/shared crates/infra crates/domain crates/app crates/entry systemprompt/src bin/bridge/src)
    gate_pass_if_empty lint-unbounded-bodies "${#ROOTS[@]}"
else
    ROOTS=("$@")
fi

EXISTING=()
for root in "${ROOTS[@]}"; do
    [ -e "$root" ] && EXISTING+=("$root")
done
[ "${#EXISTING[@]}" -eq 0 ] && { echo "lint-unbounded-bodies: no roots to scan" >&2; exit 2; }

RG=(rg --no-heading --line-number --type rust --glob '!**/tests/**' --glob '!**/target/**')

to_bytes=$("${RG[@]}" --multiline --multiline-dotall \
    'to_bytes\(\s*[^;]{0,200}?,\s*usize::MAX\s*\)' "${EXISTING[@]}" || true)
collect=$("${RG[@]}" --multiline \
    '(?i)\b\w*body\w*(\(\))?\s*\.collect\(\)\s*\.await' "${EXISTING[@]}" || true)

status=0
if [ -n "$to_bytes" ]; then
    echo "lint-unbounded-bodies: unbounded-to-bytes (use read_body_bounded or a named limit):"
    printf '%s\n' "$to_bytes" | sed 's/^/  /'
    status=1
fi
if [ -n "$collect" ]; then
    echo "lint-unbounded-bodies: unbounded-collect (wrap in http_body_util::Limited or use read_body_bounded):"
    printf '%s\n' "$collect" | sed 's/^/  /'
    status=1
fi
[ "$status" -eq 0 ] && echo "lint-unbounded-bodies: OK"
exit "$status"
