#!/usr/bin/env bash
# Every helper the bridge UI borrows from another module must be imported into
# the file that uses it, and nothing may shadow one of those names.
#
# The web tree is plain ES modules with no bundler and no type checker, so an
# identifier that was never imported is not a build error -- it is a
# ReferenceError the first time the line runs, and because each component
# renders inside its own `connectedCallback`, it takes that whole pane down
# without stopping the rest of the app. That is what `sp-profile.js` and
# `sp-activity-log.js` did: both called `t(...)` with no `import { t }`, both
# threw "t is not defined" on every fixture, and both panes were dead on
# arrival while everything around them looked fine.
#
# `lint-bridge-i18n.sh` cannot catch this. It checks that message ids exist and
# that calls carry an English fallback -- not whether `t` is in scope.
#
# The shadow check exists because of how the profile bug hid. A local
# `const t = Date.parse(iso)` in one helper meant any scan looking for "is the
# name declared in this file?" saw one and stayed quiet, while every other
# function in the file called a `t` that did not exist. A parameter is worse
# still: `(t) => ...` in a file that also calls `t("id")` makes the same three
# characters a translation function on one line and a DOM element on the next.
#
# Scope: names a module exports, and that another module therefore has to
# import. Comments, quoted strings and template-literal prose are stripped
# first, so a comment mentioning `hostStatus()` and a path ending `/bridge.js`
# are not uses. Template `${...}` expressions are kept, but nested braces inside
# one are not parsed -- that direction only ever misses a use, never invents one.
set -euo pipefail

cd "$(dirname "$0")/.."

js="bin/bridge/web/js"

strip_noise() {
    perl -0pe '
        s{/\*.*?\*/}{$& =~ tr/\n//cdr}gse;
        s{(^|[^:"'"'"'])//[^\n]*}{$1}mg;
        s{"(\\.|[^"\\\n])*"}{""}g;
        s{'"'"'(\\.|[^'"'"'\\\n])*'"'"'}{'"''"'}g;
        s{`((?:\\.|[^`\\])*)`}{ join " ", ($1 =~ /\$\{([^{}]*)\}/g) }ge;
    ' "$1"
}

# Default exports and re-exports are not used anywhere in this tree; if that
# changes, this is the line to extend.
exported=$(
    grep -rhoE '^export (async )?(function|class|const|let) [A-Za-z_$][A-Za-z0-9_$]*' "$js" \
        | awk '{print $NF}' | sort -u
)

# One alternation rather than one grep per name: 105 names across 45 files is
# 4,700 greps and half a minute of wall clock for a gate that has to be cheap
# enough to keep in `just check`.
# Why: `-s -d '|' -` rather than `-sd'|'`. BSD paste (macOS) rejects the bundled
# flag and needs an explicit `-` for stdin — without it the gate dies on its own
# usage message and reports a false red to anyone verifying locally on a Mac.
# CI is Ubuntu, so this stayed invisible there. See CLAUDE.md rule 7.
names_alt=$(printf '%s' "$exported" | paste -s -d '|' -)

fail=0
unresolved_report=""
shadow_report=""

while IFS= read -r file; do
    src=$(strip_noise "$file")

    imported=$(
        grep -ozE 'import[^;]*from[^;]*;' "$file" | tr '\0' '\n' \
            | sed -E 's/from.*//' | grep -oE '[A-Za-z_$][A-Za-z0-9_$]*' \
            | grep -vxE 'import|as' | sort -u || true
    )
    # Column 0 only: a declaration nested inside a function does not put the
    # name in scope for the rest of the file, which is the whole point.
    declared=$(
        printf '%s' "$src" \
            | grep -oE '^(export )?(async )?(function|class|const|let|var) [A-Za-z_$][A-Za-z0-9_$]*' \
            | awk '{print $NF}' | sort -u || true
    )
    used=$(
        printf '%s' "$src" \
            | grep -oP '(?<![.\w$])[A-Za-z_$][A-Za-z0-9_$]*(?![\w$:])' | sort -u || true
    )

    unresolved=$(
        comm -12 <(printf '%s\n' "$exported") <(printf '%s\n' "$used") \
            | comm -23 - <(printf '%s\n' "$imported") \
            | comm -23 - <(printf '%s\n' "$declared")
    )
    for name in $unresolved; do
        line=$(grep -nP "(?<![.\w\$])$name(?![\w\$])" "$file" | head -1 | cut -d: -f1)
        unresolved_report="$unresolved_report    $file:$line -- '$name' is used but never imported
"
        fail=1
    done

    while IFS= read -r hit; do
        [ -n "$hit" ] || continue
        name=$(printf '%s' "$hit" \
            | sed -E 's/^[0-9]+:[[:space:]]*(const|let|var|function|class)[[:space:]]+([A-Za-z_$][A-Za-z0-9_$]*).*/\2/')
        if printf '%s\n' "$exported" | grep -qx "$name"; then
            shadow_report="$shadow_report    $file:$hit
"
            fail=1
        fi
    done < <(grep -nE '^[[:space:]]+(const|let|var|function|class)[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*' "$file" || true)

    while IFS= read -r hit; do
        [ -n "$hit" ] || continue
        shadow_report="$shadow_report    $file:$hit
"
        fail=1
    done < <(grep -nE "\(($names_alt)\)[[:space:]]*=>|\(($names_alt)[[:space:]]*,|,[[:space:]]*(($names_alt))\)[[:space:]]*=>|function[[:space:]]*\([^)]*\b($names_alt)\b[^)]*\)" "$file" || true)
done < <(find "$js" -name '*.js' | sort)

if [ -n "$unresolved_report" ]; then
    echo "lint-bridge-js-imports: helper used without importing it:"
    printf '%s' "$unresolved_report"
    echo
    echo "Add the import. Without it this is a ReferenceError that kills the pane."
    echo
fi

if [ -n "$shadow_report" ]; then
    echo "lint-bridge-js-imports: local name shadows a module-scope helper:"
    printf '%s' "$shadow_report"
    echo
    echo "Rename the local. A shadow hides a missing import from every reader."
    echo
fi

[ "$fail" -eq 0 ] || exit 1

echo "lint-bridge-js-imports: OK ($(printf '%s\n' "$exported" | wc -l) exported helpers, all resolved at every use)"
