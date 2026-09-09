#!/usr/bin/env bash
# Fails when a published version string drifts from `[workspace.package].version`
# in the root Cargo.toml.
#
# Install snippets, the compatibility matrix and the stability contract are read
# by people deciding what to pin, so a stale `0.48` there is a wrong answer that
# no compiler or test can see. The bridge is versioned separately, so its own
# `version` is exempt -- but the core crates it pins are not.
#
# Prose is checked for the `0.N.x` release-line form only: exact triples appear
# throughout as IP addresses, upstream crate versions and revision-log entries,
# and a gate that flagged those would be turned off within a week. A line that
# deliberately names an older line (a historical table row, a worked example)
# carries a `version-ok` marker; anything else must match.
#
# Only git-tracked files are walked: a README that has not been `git add`ed is
# invisible to this gate, as it is to every other script gate here.
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=$(awk '/^\[workspace\.package\]/ {p=1; next} /^\[/ {p=0} p && /^version *=/ {gsub(/[" ]/, ""); sub(/^version=/, ""); print; exit}' Cargo.toml)
if [ -z "$VERSION" ]; then
    echo "check-version-strings: could not read [workspace.package].version from Cargo.toml" >&2
    exit 1
fi
MINOR="${VERSION%.*}"

mapfile -t FILES < <(git ls-files \
    'README.md' \
    'AGENTS.md' \
    'documentation/**' \
    'bin/bridge/Cargo.toml' \
    'systemprompt/src/lib.rs' \
    'systemprompt/README.md' \
    'crates/*/*/README.md')

[ "${#FILES[@]}" -gt 0 ] || { echo "check-version-strings: no tracked files in scope" >&2; exit 1; }

findings=$(awk -v version="$VERSION" -v minor="$MINOR" '
function report(found,    _) {
    printf "  %s:%d\n    expected %s (or %s), found %s\n    %s\n", FILENAME, FNR, version, minor, found, $0
    bad = 1
}
index($0, "version-ok") { next }
{
    pin = ""
    line = $0
    if (match(line, /systemprompt[a-z-]*[ \t]*=[ \t]*\{[^}]*version[ \t]*=[ \t]*"[^"]+"/)) {
        pin = substr(line, RSTART, RLENGTH)
        sub(/.*version[ \t]*=[ \t]*"/, "", pin)
        sub(/".*/, "", pin)
    } else if (match(line, /systemprompt[a-z-]*[ \t]*=[ \t]*"[0-9][^"]*"/)) {
        pin = substr(line, RSTART, RLENGTH)
        sub(/^[^"]*"/, "", pin)
        sub(/".*$/, "", pin)
    }

    if (pin != "" && pin != version && pin != minor) {
        report(pin)
        next
    }

    if (FILENAME ~ /Cargo\.toml$/) { next }

    rest = line
    while (match(rest, /0\.[0-9]+\.x/)) {
        found = substr(rest, RSTART, RLENGTH)
        rest = substr(rest, RSTART + RLENGTH)
        if (found == minor ".x") { continue }
        report(found)
    }
}
END { exit 0 }
' "${FILES[@]}")

if [ -n "$findings" ]; then
    echo "check-version-strings: version strings disagree with [workspace.package].version = $VERSION" >&2
    printf '%s\n' "$findings" >&2
    echo "Fix the file, or mark a deliberately historical line with a 'version-ok' comment." >&2
    exit 1
fi

echo "check-version-strings: all version strings match $VERSION"
