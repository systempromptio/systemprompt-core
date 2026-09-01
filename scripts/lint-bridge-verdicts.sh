#!/usr/bin/env bash
# The bridge GUI never decides what a state *means*. Every state on the wire
# ships beside a verdict the bridge computed (`src/verdict.rs`); the front end
# renders `tone` and looks `code` up in the catalogue. A JavaScript branch on a
# state's *name* is the bug class this gate exists for: the Home card once
# tested the MCP auth state against a variant that did not exist and told
# users four healthy servers were broken, while Status -- reading the same
# snapshot with its own copy of the derivation -- said they were fine.
#
# Rule: no comparison of a `.state`, `.kind`, `.phase`, `.level` or
# `.app_installed` field against a string literal, and no `switch` on one,
# anywhere under web/js. The lookup module is the one place presentation maps
# live, and it maps verdict codes, not states. Fields that are the front end's
# own (a fetcher's `state`, a toast's `kind`, an activity entry's log level)
# are listed below with the reason each is not wire state.
set -euo pipefail

cd "$(dirname "$0")/.."

web="bin/bridge/web/js"
fail=0

# receiver.field === "literal"  |  receiver.field !== "literal"  |  switch (receiver.field)
pattern='\.(state|kind|phase|level|app_installed)[[:space:]]*(===|!==|==|!=)[[:space:]]*["'"'"']|switch[[:space:]]*\([^)]*\.(state|kind|phase|level)[[:space:]]*\)'

# receiver:file pairs that are legitimately not wire state.
allow='this.state:services/marketplace-service.js
self.state:services/marketplace-service.js
this.state:components/sp-marketplace-list.js
this._fetcher.state:components/sp-marketplace.js
this.kind:components/sp-toast.js
this.kind:components/sp-marketplace-detail.js
entry.level:utils/log-format.js'

while IFS= read -r hit; do
    file=${hit%%:*}
    rest=${hit#*:}
    line=${rest%%:*}
    code=${rest#*:}
    rel=${file#"$web"/}
    receiver=$(printf '%s' "$code" | grep -oE '[A-Za-z_$][A-Za-z0-9_$.]*\.(state|kind|phase|level|app_installed)[[:space:]]*(===|!==|==|!=|\))' | head -1 | sed -E 's/[[:space:]]*(===|!==|==|!=|\))$//')
    key="${receiver}:${rel}"
    if printf '%s\n' "$allow" | grep -qxF "$key"; then
        continue
    fi
    echo "lint-bridge-verdicts: $rel:$line branches on a wire state's name: $(printf '%s' "$code" | sed 's/^[[:space:]]*//')" >&2
    fail=1
done < <(grep -rnE "$pattern" "$web" --include='*.js' || true)

if [ "$fail" -ne 0 ]; then
    echo "lint-bridge-verdicts: the derivation belongs in Rust beside the enum (src/verdict.rs); ship a tone/code and render that" >&2
    exit 1
fi
echo "lint-bridge-verdicts: OK (no JavaScript branch on a wire state name)"
