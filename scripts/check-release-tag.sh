#!/usr/bin/env bash
# Every released version must carry its tag.
#
# 0.34.0 was published to crates.io and never tagged. Nothing noticed: the
# build, clippy, every shard and the whole of CI pass against a missing tag,
# and under the next-branch flow the tag is always a separate manual step --
# cut on `main` after the release PR merges, before publishing.
# The cost lands later, when the only way left to answer
# "which commit is this published version?" is to download the .crate and diff
# it against candidate commits.
#
# The invariant, checked with no network and no registry call: a version that
# appears in CHANGELOG.md and is *older* than the workspace version must have a
# `v<version>` tag. Once the workspace has moved past a version, that version
# was released, so its tag has to exist. The version currently in the manifest
# is deliberately exempt -- the changelog entry is written before the bump, so
# requiring its tag would fail every release in progress.
set -uo pipefail

cd "$(dirname "$0")/.."

CHANGELOG="CHANGELOG.md"
MANIFEST="Cargo.toml"

# Tagging became consistent at 0.17.0; every version from there up carries its
# tag. The sixteen gaps below it (0.1.18-0.4.4, 0.11.3, 0.13.1, 0.16.1) are
# pre-convention history, and the commit a 0.1.x release was published from is
# no longer reliably recoverable. The floor is a deliberate line, not a silent
# skip: raise it never, and lower it only after tagging what it exposes.
FLOOR="0.17.0"

[ -f "$CHANGELOG" ] || { echo "check-release-tag: no $CHANGELOG" >&2; exit 2; }

CURRENT=$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*=/{gsub(/[[:space:]"]/,"");sub(/^version=/,"");print;exit}' "$MANIFEST")
[ -n "$CURRENT" ] || { echo "check-release-tag: could not read workspace version" >&2; exit 2; }

# A shallow CI checkout carries no tags, and fetching them all just to read
# their names would drag the whole history down. Only the names are needed, so
# fall back to the remote's ref list rather than requiring `fetch-depth: 0`.
TAGS=$(git tag 2>/dev/null)
if [ -z "$TAGS" ]; then
    TAGS=$(git ls-remote --tags --refs origin 2>/dev/null | sed 's|.*refs/tags/||')
    [ -n "$TAGS" ] || { echo "check-release-tag: no tags locally and none readable from origin" >&2; exit 2; }
fi

has_tag() {
    printf '%s\n' "$TAGS" | grep -qxF "$1"
}

older_than_current() {
    [ "$1" != "$CURRENT" ] && [ "$(printf '%s\n%s\n' "$1" "$CURRENT" | sort -V | head -1)" = "$1" ]
}

at_or_above_floor() {
    [ "$(printf '%s\n%s\n' "$1" "$FLOOR" | sort -V | head -1)" = "$FLOOR" ]
}

MISSING=""
CHECKED=0
while IFS= read -r version; do
    older_than_current "$version" || continue
    at_or_above_floor "$version" || continue
    CHECKED=$((CHECKED + 1))
    has_tag "v${version}" && continue
    MISSING="${MISSING}  v${version} — released (CHANGELOG) but no tag"$'\n'
done < <(grep -oE '^## \[?[0-9]+\.[0-9]+\.[0-9]+\]?' "$CHANGELOG" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -u -V)

if [ -n "$MISSING" ]; then
    echo "check-release-tag: released versions with no tag:"
    echo ""
    printf '%s' "$MISSING"
    echo ""
    echo "Tag the commit the release was published from, then push it:"
    echo "    git tag v<version> <commit> && git push origin v<version>"
    echo ""
    echo "If the commit is not obvious, the published crate settles it — download"
    echo "https://static.crates.io/crates/systemprompt-models/systemprompt-models-<version>.crate"
    echo "and diff its sources against the candidates."
    exit 1
fi

echo "check-release-tag: OK ($CHECKED released version(s) >= $FLOOR tagged; $CURRENT in flight)"
