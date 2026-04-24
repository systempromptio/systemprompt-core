#!/usr/bin/env bash
# End-to-end crates.io release for the systemprompt-core workspace.
#
# Usage:
#   ci/release.sh <patch|minor|major>
#
# Steps:
#   1. Refuse to run on a dirty tree or wrong branch.
#   2. Verify fmt, `cargo check --workspace` offline.
#   3. `cargo ws version --all <bump> --no-git-push --yes`.
#   4. Sync [workspace.dependencies] pins (cargo-ws drops stragglers).
#   5. `cargo update --workspace` to refresh Cargo.lock.
#   6. Amend the cargo-ws commit with the lock + pin fixups.
#   7. Push main + only the `vX.Y.Z` semver tag (never `--tags`).
#   8. `cargo ws publish`.
set -euo pipefail

BUMP="${1:-}"
case "$BUMP" in
  patch|minor|major) ;;
  *) echo "usage: $0 <patch|minor|major>" >&2; exit 2 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
  echo "error: must be on main, currently on $BRANCH" >&2; exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is dirty, commit or stash first" >&2; exit 1
fi

echo "==> fmt check"
cargo fmt --all -- --check

echo "==> cargo check --workspace (offline)"
SQLX_OFFLINE=true cargo check --workspace --all-targets

echo "==> cargo ws version --all $BUMP"
cargo ws version --all "$BUMP" --no-git-push --yes --no-individual-tags

OLD_VERSION_TAG="$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || echo '')"
NEW_VERSION="$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version/{gsub(/["[:space:]=]/,"",$2);print $2;exit}' Cargo.toml)"
NEW_TAG="v${NEW_VERSION}"

echo "==> sync workspace dep pins -> $NEW_VERSION"
"$SCRIPT_DIR/sync-workspace-deps.sh" Cargo.toml

echo "==> cargo update --workspace"
cargo update --workspace >/dev/null

echo "==> amending release commit with pin + lockfile fixups"
git add Cargo.toml Cargo.lock
git commit --amend --no-edit

# cargo-ws leaves a `vX.Y.Z` (or crate-scoped) tag; make sure the one we
# want exists on the amended commit and drop any stale reference.
git tag -d "$NEW_TAG" 2>/dev/null || true
git tag "$NEW_TAG"

echo "==> push main + $NEW_TAG (semver only, never --tags)"
git push origin main
git push origin "$NEW_TAG"

echo "==> cargo ws publish"
cargo ws publish --no-verify --publish-as-is --yes --from-git

echo ""
echo "Released $NEW_TAG."
echo "Next: bump ../systemprompt-template's systemprompt dep to $NEW_VERSION."
