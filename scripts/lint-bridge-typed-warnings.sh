#!/usr/bin/env bash
# Control flow in the bridge decides on a warning's `kind`, never on its
# message text. The Cowork re-sync tick once matched "any warning under
# claude-desktop" and re-requested a sync every 30 s for as long as a feedback
# timeout kept a warning there. A `host_warnings.iter().any(..)` that does not
# read `.kind`, or any comparison on `.message`, fails here. No allowlist.
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
src="$root/bin/bridge/src"
status=0

if ! command -v rg >/dev/null 2>&1; then
    echo "lint-bridge-typed-warnings: ripgrep (rg) is required" >&2
    exit 2
fi

hits=$(rg -n --no-heading --color=never -g '*.rs' \
    '\.message\.(contains|starts_with|ends_with|eq|eq_ignore_ascii_case)\(|\.message\s*==' "$src" || true)
if [ -n "$hits" ]; then
    echo "lint-bridge-typed-warnings: a decision reads a warning's message text; use its kind:"
    echo "$hits"
    status=1
fi

hits=$(find "$src" -name '*.rs' -print0 | xargs -0 perl -0777 -ne '
    while (/host_warnings\s*\.iter\(\)\s*\.any\(\s*\|[^|]*\|\s*(\{(?:[^{}]|\{[^{}]*\})*\}|[^)]*)\)/g) {
        my $body = $1; my $pos = pos();
        my $line = 1 + (substr($_, 0, $pos) =~ tr/\n//);
        print "$ARGV:$line: host_warnings predicate never reads .kind\n" unless $body =~ /\.kind/;
    }' || true)
if [ -n "$hits" ]; then
    echo "lint-bridge-typed-warnings: a host_warnings predicate that never reads .kind:"
    echo "$hits"
    status=1
fi

if [ "$status" -ne 0 ]; then
    exit 1
fi
echo "lint-bridge-typed-warnings: OK"
