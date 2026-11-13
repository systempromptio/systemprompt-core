#!/usr/bin/env bash
# The bridge web tree is plain ES modules and plain CSS with no bundler, no
# type checker and no test runner: a file is reviewed by reading it top to
# bottom, and its blast radius when it breaks is the whole pane it renders.
# The front-end standard therefore caps a JS module at 150 lines and a
# stylesheet at 300, and asks that anything larger be split by responsibility,
# not trimmed.
#
# The caps were on paper only. Every pane component grew past them in turn --
# sp-agent-drawer.js reached 480 lines, profile.css 541 -- and each one was
# split in a hurry once it became unreviewable. This gate turns the standard
# into a failure, so a file is split when it crosses the line rather than
# when it has already become the problem.
#
# `bin/bridge/web/dev/` is the dev-only fixture server and mocked IPC; it is
# not shipped and its fixture tables are legitimately long.
#
#   scripts/lint-bridge-file-size.sh [--paths P...]
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
gate_scope_parse "$@"
set -- "${GATE_ARGS[@]}"
cd "$GATE_ROOT"

js_limit=150
css_limit=300

fail=0

while IFS= read -r file; do
    case "$file" in
        bin/bridge/web/dev/*) continue ;;
        bin/bridge/web/js/*.js) limit=$js_limit ;;
        bin/bridge/web/css/*.css) limit=$css_limit ;;
        *) continue ;;
    esac
    n=$(wc -l < "$file")
    if [ "$n" -gt "$limit" ]; then
        echo "$file: $n lines (limit $limit)"
        fail=1
    fi
done < <(gate_files 'bin/bridge/web/js' 'bin/bridge/web/css')

if [ "$fail" -ne 0 ]; then
    echo
    echo "lint-bridge-file-size: split the file by responsibility (see javascript-coding-standards / css-coding-standards)."
    exit 1
fi

echo "lint-bridge-file-size: OK"
