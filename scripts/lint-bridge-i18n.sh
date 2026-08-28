#!/usr/bin/env bash
# Every message id the bridge UI references must exist in the en-US catalogue,
# every catalogue key must be referenced, and every literal `t("id")` must carry
# an English fallback.
#
# `web/js/i18n.js`'s `t()` used to return the *id* when a key was missing. An id
# is a truthy string, so all 117 `t("id") || "English"` fallbacks in the tree were
# unreachable and a missing key rendered its own id at the user -- which is what
# `status-cloud-reach-label` and `status-cloud-identity-label` did on the shipped
# Status pane, printing "status-cloud-reach-label" where "Reachability" belongs.
# `t()` now returns undefined, so the fallbacks work; this gate is what keeps them
# present and keeps the catalogue and the tree in agreement.
#
# The catalogue is shared with Rust (`src/i18n.rs` embeds it via include_str!),
# so references are collected from `src/**.rs` as well as the web tree.
set -euo pipefail

cd "$(dirname "$0")/.."

bridge="bin/bridge"
web="$bridge/web"
ftl="$web/i18n/en-US/bridge.ftl"

# Ids built by interpolation cannot be read off the source, so each family is
# declared here and every catalogue key under the prefix counts as referenced.
# Adding a family means adding it to this list -- deliberately awkward, because
# an unlisted interpolated id is unverifiable.
dynamic_prefixes="agent-action- setup-install-stage-"

# --- definitions -------------------------------------------------------------
defined=$(grep -oE '^[a-z][a-z0-9-]* *=' "$ftl" | sed 's/ *=//' | sort -u)

# --- references --------------------------------------------------------------
# Literal ids: t("id") and the drawer's _section("id"), the three data-l10n-*
# attributes, TAB_DEFS' `l10n:` field, and the *_L10N indirection maps.
referenced=$(
    {
        grep -rhoE '\bt\(\s*"[a-z][a-z0-9-]*"' "$web" || true
        grep -rhoE '_section\(\s*"[a-z][a-z0-9-]*"' "$web" || true
        grep -rhoE 'data-l10n-(id|aria|placeholder)="[a-z][a-z0-9-]*"' "$web" || true
        grep -rhoE '\bl10n: *"[a-z][a-z0-9-]*"' "$web" || true
        find "$web/js" -name '*.js' -exec awk '/_L10N *= *\{/,/^\};?$/' {} + \
            | grep -oE ': *"[a-z][a-z0-9-]*"' || true
        find "$bridge/src" -name '*.rs' -exec \
            grep -Phzo 'i18n::t(_args)?\(\s*"[a-z][a-z0-9-]*"' {} + | tr '\0' '\n' || true
    } | grep -oE '"[a-z][a-z0-9-]*"' | tr -d '"' | sort -u
)

fail=0

# --- 1. referenced but undefined ---------------------------------------------
missing=$(comm -13 <(echo "$defined") <(echo "$referenced"))
if [ -n "$missing" ]; then
    echo "lint-bridge-i18n: message ids referenced but absent from the catalogue:"
    for id in $missing; do
        echo "  $id"
        grep -rn "\\b$id\\b" "$web/js" "$web/index.html" "$bridge/src" | sed 's/^/    /'
    done
    echo
    echo "Add it to $ftl."
    fail=1
fi

# --- 2. defined but unreferenced ---------------------------------------------
# Deliberately looser than check 1: an id can reach `t()` through a helper's
# parameter (`_section("agent-section-health", ...)`, `item("open-settings",
# "topbar-menu-settings", ...)`), and enumerating every such helper would make
# this gate a maintenance burden that fails on correct code. Message ids are
# distinctive enough that a bare quoted occurrence anywhere is proof of life,
# and this check only exists to stop the catalogue accumulating cruft.
any_literal=$(
    { grep -rhoE '"[a-z][a-z0-9-]*"' "$web/js" "$web/index.html" || true
      find "$bridge/src" -name '*.rs' -exec grep -hoE '"[a-z][a-z0-9-]*"' {} + || true
    } | tr -d '"' | sort -u
)
referenced_with_families="$any_literal"
for prefix in $dynamic_prefixes; do
    referenced_with_families=$(
        { echo "$referenced_with_families"; echo "$defined" | grep "^$prefix" || true; } | sort -u
    )
done
dead=$(comm -23 <(echo "$defined") <(echo "$referenced_with_families"))
if [ -n "$dead" ]; then
    echo "lint-bridge-i18n: catalogue keys nothing references:"
    for id in $dead; do echo "  $id"; done
    echo
    echo "Wire it up, or delete it from $ftl."
    fail=1
fi

# --- 3. literal t("id") without an English fallback ---------------------------
# `t()` returns undefined on a miss, and a failed catalogue fetch misses every
# key, so an unguarded call renders the string "undefined" at the user.
unguarded=$(
    grep -rnE '\bt\(\s*"[a-z][a-z0-9-]*"\s*\)[^|]' "$web" \
        | grep -vE '\|\|' || true
)
if [ -n "$unguarded" ]; then
    echo "lint-bridge-i18n: t(\"id\") without an English fallback:"
    echo "$unguarded" | sed 's/^/    /'
    echo
    echo 'Write t("id") || "English text".'
    fail=1
fi

# --- 4. unrecognised interpolated ids -----------------------------------------
interpolated=$(
    grep -rnE 'data-l10n-(id|aria|placeholder)="\$\{' "$web" \
        | grep -vE '\$\{def\.l10n\}|\$\{[A-Z_]+_L10N\[' || true
)
if [ -n "$interpolated" ]; then
    echo "lint-bridge-i18n: interpolated message id this gate cannot resolve:"
    echo "$interpolated" | sed 's/^/    /'
    echo
    echo "Use a literal id, or a *_L10N lookup map."
    fail=1
fi

[ "$fail" -eq 0 ] || exit 1

echo "lint-bridge-i18n: OK ($(echo "$referenced" | wc -w) references, $(echo "$defined" | wc -w) keys, all resolved)"
