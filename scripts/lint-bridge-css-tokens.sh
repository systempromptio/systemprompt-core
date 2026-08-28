#!/usr/bin/env bash
# Every `var(--sp-*)` in the bridge UI must resolve to a definition, unless the
# reference supplies its own fallback.
#
# An undefined custom property is invalid at computed-value time: the browser
# drops the whole declaration and paints the initial value instead, silently.
# That is not a theoretical failure -- `--sp-space-5` was referenced by
# profile.css and never defined, which killed the Profile header's bottom margin
# and left a hairline sitting flush against the cards, and separately erased an
# empty state's entire padding. Neither showed up as an error anywhere.
set -euo pipefail

cd "$(dirname "$0")/.."

web="bin/bridge/web"

# Definitions: anything declared as `--sp-foo:` in any sheet, plus the two
# properties scroll.css registers with @property.
defined=$(
    { grep -rhoE '^\s*--sp-[a-z0-9-]+\s*:' "$web/css" | tr -d ' :' 
      grep -rhoE '@property\s+--sp-[a-z0-9-]+' "$web/css" | awk '{print $2}'
    } | sort -u
)

# References without a fallback, i.e. `var(--sp-foo)` and not `var(--sp-foo, x)`.
referenced=$(grep -rhoE 'var\(\s*--sp-[a-z0-9-]+\s*\)' "$web" | grep -oE -- '--sp-[a-z0-9-]+' | sort -u)

missing=$(comm -13 <(echo "$defined") <(echo "$referenced"))

if [ -n "$missing" ]; then
    echo "lint-bridge-css-tokens: undefined design tokens referenced without a fallback:"
    for token in $missing; do
        echo "  $token"
        grep -rn "var($token)" "$web" | sed 's/^/    /'
    done
    echo
    echo "Define it in $web/css/tokens.css, or give the reference a fallback."
    exit 1
fi

echo "lint-bridge-css-tokens: OK ($(echo "$referenced" | wc -w) references, all defined)"
