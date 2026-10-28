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

[ "$fail" -eq 0 ] && echo "check-no-vendor: OK (no third-party frontend code)"
exit "$fail"
