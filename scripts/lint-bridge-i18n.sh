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
#
# `setup-install-stage-` is the only genuinely unverifiable family left: its
# suffix is an error's `stage` field, invented at the throw site, so there is no
# closed set to check against.
dynamic_prefixes="setup-install-stage- update-phase-"

# The rest of the interpolated families are not unverifiable at all -- their
# suffix is a Rust enum serialised kebab-case, which IS the closed set. Each
# entry is `prefix:file:enum`, and the enum is read as the producer, so these
# families get checked in both directions instead of blanket-accepted: every
# code the bridge can emit must have a string (check 5), and a key is proof of
# life only if something can actually produce it (check 2).
#
# Blanket-accepting a prefix is what let the agent-health families sit
# unreferenced: `t(`agent-state-${state}`)` is invisible to a literal scan, so
# the gate called nineteen live keys dead and would equally have called a
# missing string fine. Prefer this list; use `dynamic_prefixes` only when there
# is no producer to point at.
enum_families="agent-state-:integration/agent_health.rs:AgentState
agent-reason-:integration/agent_health.rs:AgentReason
agent-action-:integration/agent_health.rs:AgentAction
agents-fleet-:integration/agent_fleet.rs:FleetHeadline
tone-section-:verdict.rs:Tone
gateway-state-:gui/state/verdicts.rs:GatewayCode
identity-:gui/state/verdicts.rs:IdentityCode
agents-status-cloud-:gui/state/verdicts.rs:IdentityCode
overall-:gui/state/verdicts.rs:OverallCode
agents-status-token-:gui/state/verdicts.rs:TokenCode
setup-health-label-:gui/state/verdicts.rs:HealthCode
proxy-state-:proxy_probe/mod.rs:ProxyProbeState
agents-status-proxy-:proxy_probe/mod.rs:ProxyProbeState
mcp-auth-:proxy/mcp_probe/types.rs:McpAuthState
host-profile-:integration/profile_state.rs:ProfileCode
host-app-:integration/profile_state.rs:AppInstallState
agent-kind-:integration/host_app.rs:HostKind
settings-schedule-:schedule/status.rs:ScheduleStatus"

# This reimplements `rename_all = "kebab-case"` in awk, so it is only correct
# while that is actually how the enum serialises. Rather than guess, assert it:
# a missing `rename_all` or a per-variant `#[serde(rename = ...)]` means the
# codes below would be quietly wrong, so bail loudly instead of emitting a
# confident wrong answer. `tests/agent_health_i18n.rs` checks the same coupling
# through real serde; this is the fast, compile-free half of it.
assert_kebab_serde() {
    grep -q "^[[:space:]]*pub enum $2[[:space:]]*{" "$1" \
        || { echo "lint-bridge-i18n: $1 has no \`pub enum $2\` -- it was renamed or moved; update enum_families" >&2; return 1; }
    awk -v want="$2" -v file="$1" '
        $0 ~ ("^[[:space:]]*pub enum " want "[[:space:]]*\\{") { inside = 1 }
        inside && /^\}/ { inside = 0 }
        inside && /#\[serde\([^)]*rename[[:space:]]*=/ {
            print "lint-bridge-i18n: " file " " want " has a per-variant #[serde(rename)] -- this gate assumes plain kebab-case" > "/dev/stderr"
            bad = 1
        }
        END { exit bad ? 1 : 0 }
    ' "$1" || return 1
    grep -B6 "^[[:space:]]*pub enum $2[[:space:]]*{" "$1" \
        | grep -q 'rename_all[[:space:]]*=[[:space:]]*"kebab-case"' \
        || { echo "lint-bridge-i18n: $1 $2 is not #[serde(rename_all = \"kebab-case\")] -- this gate assumes it" >&2; return 1; }
}

# Serialises one enum's variants the way `#[serde(rename_all = "kebab-case")]`
# does, so the catalogue is compared against the exact strings that reach `t()`.
variant_codes() {
    awk -v prefix="$1" -v want="$3" '
        function kebab(s,   i, c, out) {
            out = ""
            for (i = 1; i <= length(s); i++) {
                c = substr(s, i, 1)
                if (c ~ /[A-Z]/) { out = out (i > 1 ? "-" : "") tolower(c) }
                else { out = out c }
            }
            return out
        }
        $0 ~ ("^[[:space:]]*pub enum " want "[[:space:]]*\\{") { inside = 1; next }
        inside && /^\}/ { inside = 0 }
        inside {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line ~ /^[A-Z][A-Za-z0-9]*[[:space:]]*[,{(]?$/ || line ~ /^[A-Z][A-Za-z0-9]*[[:space:]]*[{(]/) {
                match(line, /^[A-Z][A-Za-z0-9]*/)
                print prefix kebab(substr(line, 1, RLENGTH))
            }
        }
    ' "$2"
}

produced=""
for family in $enum_families; do
    prefix=${family%%:*}
    rest=${family#*:}
    file="$bridge/src/${rest%%:*}"
    enum=${rest#*:}
    [ -f "$file" ] || { echo "lint-bridge-i18n: $file is gone -- update enum_families" >&2; exit 2; }
    assert_kebab_serde "$file" "$enum" || exit 2
    codes=$(variant_codes "$prefix" "$file" "$enum")
    [ -n "$codes" ] || { echo "lint-bridge-i18n: no variants read from $enum in $file -- update enum_families" >&2; exit 2; }
    produced=$(printf '%s\n%s\n' "$produced" "$codes")
done
produced=$(printf '%s' "$produced" | grep -v '^$' | sort -u)

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
        # Why: perl, not `grep -P`. PCRE is a GNU grep extension; BSD grep
        # (macOS) rejects -P outright, and because this arm ends in `|| true`
        # it failed SILENTLY there — the Rust ids simply never joined the
        # reference set, so check 1 could not catch a missing Rust-side key on
        # a Mac at all. Measured 416 references on Linux against 383 on macOS.
        # A false pass is worse than a false red: nothing tells you it happened.
        # `grep -oE` cannot replace it because \s* has to match across newlines,
        # which needs perl's -0777 slurp. Quotes are re-emitted so the shared
        # `grep -oE '"..."'` downstream still sees the same shape.
        find "$bridge/src" -name '*.rs' -exec \
            perl -0777 -ne 'print "\"$1\"\n" while /i18n::t(?:_args)?\(\s*"([a-z][a-z0-9-]*)"/g' {} + || true
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
referenced_with_families=$(printf '%s\n%s\n' "$any_literal" "$produced" | sort -u)
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

# --- 5. an emittable code with no string ---------------------------------------
# The mirror of check 2, and the reason these families are worth declaring: a
# variant added in Rust without its catalogue entry renders the English
# fallback at best, and on the interpolated paths there is no fallback to
# render -- `t(`agent-state-${state}`) || ""` is an empty label on the card.
unstringed=$(comm -13 <(echo "$defined") <(echo "$produced"))
if [ -n "$unstringed" ]; then
    echo "lint-bridge-i18n: codes the bridge can emit with no catalogue string:"
    for id in $unstringed; do echo "  $id"; done
    echo
    echo "Add each to $ftl -- the interpolated call sites have no fallback text."
    fail=1
fi

[ "$fail" -eq 0 ] || exit 1

echo "lint-bridge-i18n: OK ($(echo "$referenced" | wc -w) references, $(echo "$produced" | wc -w) emittable codes, $(echo "$defined" | wc -w) keys, all resolved)"
