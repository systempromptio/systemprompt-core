#!/usr/bin/env bash
# Core ships Rust, schemas and protocol, never third-party frontend code.
#
# Third-party JS, WASM, CSS and fonts belong to the extension that serves
# them (behind its own reviewed allowlist), so a tracked file here fails when
# it is any of:
#   - inside a `vendor/` or `node_modules/` directory
#   - a `package.json` or a `.wasm` binary
#   - markup or script loading code from an external URL
#     (`<script src="http…">`, `import … from "http…"`, `import("http…")`)
# Core's own first-party scripts (the bridge web UI) stay allowed. Rust crates
# keep following deny.toml.
#
# Core is also headless: a host extension owns its browser UI. Every tracked
# `.js`, `.mjs`, `.ts`, `.css`, `.html` or `.hbs` file under `crates/` must
# match a glob in scripts/core-markup-allowlist.txt, so new markup or script
# in a core crate is a reviewed addition to that list.
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
paths=$(git ls-files | grep -E '(^|/)(vendor|node_modules)/|(^|/)package\.json$|\.wasm$' || true)
if [ -n "$paths" ]; then
    echo "check-no-vendor: third-party frontend files are tracked in core:" >&2
    printf '  %s\n' $paths >&2
    fail=1
fi

external=$(git ls-files -z '*.js' '*.mjs' '*.html' '*.hbs' '*.css' \
    | xargs -0 -r grep -nEi \
        -e '<script[^>]*src=["'"'"']?(https?:)?//' \
        -e '(import|from)[[:space:]]*\(?[[:space:]]*["'"'"'](https?:)?//' \
        -e '@import[[:space:]]+(url\()?["'"'"']?(https?:)?//' \
    || true)
if [ -n "$external" ]; then
    echo "check-no-vendor: external code is loaded from a URL:" >&2
    printf '%s\n' "$external" >&2
    fail=1
fi

allowlist=scripts/core-markup-allowlist.txt
patterns=()
while IFS= read -r line; do
    line="${line%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [ -n "$line" ] && patterns+=("$line")
done < "$allowlist"

unlisted=()
while IFS= read -r -d '' path; do
    allowed=0
    for pattern in "${patterns[@]}"; do
        if [[ $path == $pattern ]]; then
            allowed=1
            break
        fi
    done
    [ "$allowed" -eq 0 ] && unlisted+=("$path")
done < <(git ls-files -z -- 'crates/*.js' 'crates/*.mjs' 'crates/*.ts' 'crates/*.css' 'crates/*.html' 'crates/*.hbs')
if [ "${#unlisted[@]}" -gt 0 ]; then
    echo "check-no-vendor: browser markup or script outside $allowlist:" >&2
    printf '  %s\n' "${unlisted[@]}" >&2
    fail=1
fi

[ "$fail" -eq 0 ] && echo "check-no-vendor: OK (no third-party frontend code; browser markup limited to $allowlist)"
exit "$fail"
