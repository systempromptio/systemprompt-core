#!/usr/bin/env bash
# A crate that changed must say what changed in it.
#
# The build, clippy, rustdoc, the shards and every other source gate pass
# against an undocumented crate, so this is the gate that enforces it.
#
# The invariant: a crate whose `src` changed between the newest release tag and
# HEAD is named by a changelog fragment (`changelog.d/*.toml`, see
# changelog.d/README.md), or -- once the release bump has assembled the
# fragments -- its CHANGELOG.md carries a heading for the version in its
# manifest. Between releases the fragment is the only way to satisfy it; while
# a release is in flight (manifest ahead of the newest tag) either is.
#
# A shallow checkout has neither the tag nor the history to diff, and every
# crate would read as unchanged -- a gate green because it could not run. That
# exits 2, not 0, so the failure is loud either way. The CI job carries
# `fetch-depth: 0` for this reason. An invalid fragment exits 2 as well:
# `changelog-assemble.py --check` names it.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 2

MANIFEST="Cargo.toml"

CURRENT=$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*=/{gsub(/[[:space:]"]/,"");sub(/^version=/,"");print;exit}' "$MANIFEST")
[ -n "$CURRENT" ] || { echo "check-crate-changelogs: could not read workspace version" >&2; exit 2; }

TAG=$(git tag -l 'v*' --sort=-v:refname 2>/dev/null | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | head -1)
[ -n "$TAG" ] || { echo "check-crate-changelogs: no release tag reachable -- unshallow the checkout (fetch-depth: 0)" >&2; exit 2; }

git rev-parse --quiet --verify "$TAG^{commit}" >/dev/null \
    || { echo "check-crate-changelogs: $TAG names no commit in this checkout -- unshallow it (fetch-depth: 0)" >&2; exit 2; }

git rev-parse --quiet --verify HEAD >/dev/null \
    || { echo "check-crate-changelogs: no HEAD commit to diff" >&2; exit 2; }

LATEST=${TAG#v}

IN_FLIGHT=1
[ "$CURRENT" = "$LATEST" ] && IN_FLIGHT=0

if [ "$IN_FLIGHT" -eq 1 ] && [ "$(printf '%s\n%s\n' "$CURRENT" "$LATEST" | sort -V | head -1)" = "$CURRENT" ]; then
    echo "check-crate-changelogs: workspace version $CURRENT is older than the newest tag $TAG" >&2
    exit 2
fi

NAMED=$(python3 scripts/changelog-assemble.py --named-crates) \
    || { echo "check-crate-changelogs: changelog fragments do not validate (scripts/changelog-assemble.py --check)" >&2; exit 2; }

if ! CHANGED=$(git diff --name-only "$TAG" HEAD -- 'crates/*/src/*' 2>/dev/null); then
    echo "check-crate-changelogs: cannot diff $TAG..HEAD -- unshallow the checkout (fetch-depth: 0)" >&2
    exit 2
fi

# The crate root is the path above `src`, so a crate nested any depth under
# `crates/` resolves the same way and no crate list has to be kept in step.
CRATES=$(printf '%s\n' "$CHANGED" | sed -n 's|\(.*\)/src/.*|\1|p' | sort -u)

MISSING=""
CHECKED=0
while IFS= read -r crate; do
    [ -n "$crate" ] || continue
    [ -f "$crate/Cargo.toml" ] || continue
    grep -q '^\[package\]' "$crate/Cargo.toml" || continue

    # An unpublished crate has no release to document, and the repository
    # forbids markdown under `crates/**` outside README/CHANGELOG anyway. This
    # is what keeps the whole of `crates/tests` out without naming that path.
    grep -qE '^publish[[:space:]]*=[[:space:]]*false' "$crate/Cargo.toml" && continue

    VERSION=$(awk '/^\[package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*[=.]/{print;exit}' "$crate/Cargo.toml")
    case "$VERSION" in
        *workspace*) VERSION="$CURRENT" ;;
        *) VERSION=$(printf '%s' "$VERSION" | sed 's/[[:space:]"]//g; s/^version=//') ;;
    esac
    [ -n "$VERSION" ] || { echo "check-crate-changelogs: could not read version of $crate" >&2; exit 2; }

    NAME=$(awk '/^\[package\]/{p=1;next}/^\[/{p=0}p&&/^name[[:space:]]*=/{gsub(/[[:space:]"]/,"");sub(/^name=/,"");print;exit}' "$crate/Cargo.toml")
    [ -n "$NAME" ] || { echo "check-crate-changelogs: could not read package name of $crate" >&2; exit 2; }

    CHECKED=$((CHECKED + 1))

    grep -qxF "$NAME" <<< "$NAMED" && continue

    if [ "$IN_FLIGHT" -eq 1 ] && [ -f "$crate/CHANGELOG.md" ] \
        && grep -qE "^##[[:space:]]+\[?${VERSION//./\\.}\]?([[:space:]]|$)" "$crate/CHANGELOG.md"; then
        continue
    fi

    if [ "$IN_FLIGHT" -eq 1 ]; then
        MISSING="$MISSING\n  $crate ($NAME) -- no changelog.d fragment names it and CHANGELOG.md has no heading for $VERSION"
    else
        MISSING="$MISSING\n  $crate ($NAME) -- no changelog.d fragment names it"
    fi
done <<< "$CRATES"

if [ -n "$MISSING" ]; then
    echo "check-crate-changelogs: crates changed since $TAG with nothing to show for it:" >&2
    printf '%b\n' "$MISSING" >&2
    echo "" >&2
    echo "Add a changelog.d/<id>.toml fragment with a [[crate]] entry for each (changelog.d/README.md)," >&2
    echo "describing that crate's own diff since $TAG." >&2
    exit 1
fi

if [ "$IN_FLIGHT" -eq 1 ]; then
    echo "check-crate-changelogs: $CHECKED crate(s) changed since $TAG, each named by a fragment or a $CURRENT heading"
else
    echo "check-crate-changelogs: $CHECKED crate(s) changed since $TAG, each named by a fragment"
fi
