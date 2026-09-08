#!/usr/bin/env bash
# A crate that changed must say what changed in it.
#
# 0.42.0 shipped source changes in ten crates and a CHANGELOG entry in none of
# them. Nothing caught it: the build, clippy, rustdoc, all 14 shards and every
# other source gate pass against an undocumented crate, and all 53 checks on
# the release PR were green. The rule existed only as prose in
# release-flow.md 3, and prose is not a gate. The decay was visible in the
# tree -- `domain/ai` last carried an entry at 0.38.0, `domain/analytics` at
# 0.31.0.
#
# The invariant: a crate whose `src` changed between the newest release tag and
# HEAD must carry a heading for the version in its manifest.
#
# It only fires while a release is in flight -- the manifest ahead of the newest
# tag. Between releases `next` sits at the version already published and there
# is no entry to name, so demanding one would fail every push and be silenced
# within a day. On the not-pre-bumped path the bump and the entries land in the
# same commit, which is exactly the commit this checks.
#
# A shallow checkout has neither the tag nor the history to diff, and every
# crate would read as unchanged -- a gate green because it could not run. That
# exits 2, not 0, so the failure is loud either way. The CI job carries
# `fetch-depth: 0` for this reason.
set -uo pipefail

cd "$(dirname "$0")/.."

MANIFEST="Cargo.toml"

CURRENT=$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*=/{gsub(/[[:space:]"]/,"");sub(/^version=/,"");print;exit}' "$MANIFEST")
[ -n "$CURRENT" ] || { echo "check-crate-changelogs: could not read workspace version" >&2; exit 2; }

TAG=$(git tag -l 'v*' --sort=-v:refname 2>/dev/null | head -1)
[ -n "$TAG" ] || { echo "check-crate-changelogs: no release tag reachable -- unshallow the checkout (fetch-depth: 0)" >&2; exit 2; }

git rev-parse --quiet --verify "$TAG^{commit}" >/dev/null \
    || { echo "check-crate-changelogs: $TAG names no commit in this checkout -- unshallow it (fetch-depth: 0)" >&2; exit 2; }

git rev-parse --quiet --verify HEAD >/dev/null \
    || { echo "check-crate-changelogs: no HEAD commit to diff" >&2; exit 2; }

LATEST=${TAG#v}

if [ "$CURRENT" = "$LATEST" ]; then
    echo "check-crate-changelogs: workspace at $CURRENT, the version $TAG already published -- no release in flight, nothing to name"
    exit 0
fi

if [ "$(printf '%s\n%s\n' "$CURRENT" "$LATEST" | sort -V | head -1)" = "$CURRENT" ]; then
    echo "check-crate-changelogs: workspace version $CURRENT is older than the newest tag $TAG" >&2
    exit 2
fi

CHANGED=$(git diff --name-only "$TAG" HEAD -- 'crates/*/src/*' 2>/dev/null)
if [ $? -ne 0 ]; then
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

    CHECKED=$((CHECKED + 1))

    if [ ! -f "$crate/CHANGELOG.md" ]; then
        MISSING="$MISSING\n  $crate -- no CHANGELOG.md (needs a $VERSION entry)"
        continue
    fi

    grep -qE "^##[[:space:]]+\[?${VERSION//./\\.}\]?([[:space:]]|$)" "$crate/CHANGELOG.md" \
        || MISSING="$MISSING\n  $crate -- CHANGELOG.md has no heading for $VERSION"
done <<< "$CRATES"

if [ -n "$MISSING" ]; then
    echo "check-crate-changelogs: crates changed since $TAG with nothing to show for it:" >&2
    printf '%b\n' "$MISSING" >&2
    echo "" >&2
    echo "Add a '## [<version>] - <date>' section to each, describing that crate's own diff since $TAG." >&2
    exit 1
fi

echo "check-crate-changelogs: $CHECKED crate(s) changed since $TAG, all carry a $CURRENT entry"
