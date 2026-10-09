#!/usr/bin/env bash
# Prepares the release BUMP COMMIT for the systemprompt-core workspace, on
# `next` or on a detached worktree whose HEAD is origin/next (`just worktree`).
#
# This script does not tag, push or publish: `main` is protected (PR-only, no
# bypass), so the release lands via `just gate` -> `just promote` -> merge ->
# tag -> publish.
#
# What it does: on a clean tree at origin/next that already contains origin/main
# and whose push run is green in the cloud, bump the workspace, sync the
# dependency pins, facade snippets and bridge pins, refresh every tracked
# workspace lockfile, require the version strings to agree, and fold it into one
# release commit. No local build or test: the push run of the release commit is
# the proof.
#
# Usage:
#   scripts/release.sh <patch|minor|major> [--dry-run]
#
# --dry-run performs every check and lists the files the bump would rewrite;
# it changes nothing.
set -euo pipefail

BUMP=""
DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    patch|minor|major) BUMP="$arg" ;;
    --dry-run) DRY_RUN=1 ;;
    *) echo "usage: $0 <patch|minor|major> [--dry-run]" >&2; exit 2 ;;
  esac
done
[[ -n "$BUMP" ]] || { echo "usage: $0 <patch|minor|major> [--dry-run]" >&2; exit 2; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

LOCK_MANIFESTS=(. bin/bridge crates/tests crates/tests/bench crates/tests/fuzz)

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "next" && "$BRANCH" != "HEAD" ]]; then
  echo "error: releases are built on next or a detached origin/next worktree, currently on $BRANCH" >&2; exit 1
fi
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "error: tracked files are modified, commit or restore them first:" >&2
  git status --short --untracked-files=no >&2; exit 1
fi

git fetch -q origin next main
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse origin/next)" ]]; then
  echo "error: HEAD is not origin/next — pull (or recreate the worktree) first" >&2; exit 1
fi
if ! git merge-base --is-ancestor origin/main HEAD; then
  echo "error: origin/main is not an ancestor of HEAD — the release PR would not fast-forward." >&2
  echo "       fix: git merge origin/main on next, push, and wait for its push run" >&2; exit 1
fi
echo "==> next must already be green in the cloud (CI, Quality, Supply Chain push runs)"
python3 "$SCRIPT_DIR/release-proof.py" pushed HEAD

# cargo-ws is deliberately not used: `cargo ws version` updates the lockfile
# BEFORE the workspace dep pins are synced, so it dies on its own half-bumped
# state ("failed to select a version for systemprompt-identifiers"). The bump
# is a single [workspace.package].version line; everything else is the sync
# script and cargo update, in an order that is never inconsistent.
OLD_VERSION="$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*=/{gsub(/[[:space:]"]/,""); sub(/^version=/,""); print; exit}' Cargo.toml)"
IFS=. read -r MAJ MIN PAT <<<"$OLD_VERSION"
case "$BUMP" in
  patch) NEW_VERSION="$MAJ.$MIN.$((PAT+1))" ;;
  minor) NEW_VERSION="$MAJ.$((MIN+1)).0" ;;
  major) NEW_VERSION="$((MAJ+1)).0.0" ;;
esac

RELEASE_FILES=(Cargo.toml bin/bridge/Cargo.toml systemprompt/src/lib.rs systemprompt/README.md)
for m in "${LOCK_MANIFESTS[@]}"; do RELEASE_FILES+=("${m#./}/Cargo.lock"); done
RELEASE_FILES=("${RELEASE_FILES[@]#./}")

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "==> version strings (current tree)"
  just check-version-strings
  echo "==> dry run: $OLD_VERSION -> $NEW_VERSION would rewrite:"
  printf '    %s\n' "${RELEASE_FILES[@]}"
  exit 0
fi

echo "==> bump workspace $OLD_VERSION -> $NEW_VERSION"
sed -i "0,/^version = \"$OLD_VERSION\"/s//version = \"$NEW_VERSION\"/" Cargo.toml

echo "==> sync workspace dep pins -> $NEW_VERSION"
"$SCRIPT_DIR/sync-workspace-deps.sh" Cargo.toml

echo "==> sync facade version snippets -> $NEW_VERSION"
sed -i -E "s/(systemprompt = \{ version = \")[^\"]+(\")/\1${NEW_VERSION}\2/" \
  systemprompt/src/lib.rs systemprompt/README.md

echo "==> bridge version and its core pins -> $NEW_VERSION"
sed -i -E "1,80s/\"$OLD_VERSION\"/\"$NEW_VERSION\"/" bin/bridge/Cargo.toml

echo "==> cargo update --workspace (${LOCK_MANIFESTS[*]})"
for m in "${LOCK_MANIFESTS[@]}"; do
  cargo update --workspace --manifest-path "$m/Cargo.toml" >/dev/null
done

echo "==> version strings"
if ! just check-version-strings; then
  echo "error: version strings disagree with $NEW_VERSION (internal/release.md §1c). Nothing is committed;" >&2
  echo "       the bump is in the tree. Fix the strings, then: just check-version-strings &&" >&2
  echo "       git add ${RELEASE_FILES[*]} <fixed files> && git commit -m \"Release $NEW_VERSION\"" >&2; exit 1
fi

echo "==> release commit"
git add "${RELEASE_FILES[@]}"
git commit -m "Release $NEW_VERSION"

cat <<EOF

Bumped to ${NEW_VERSION} at $(git rev-parse --short HEAD). Still manual (internal/release.md §1c):
root README.md, AGENTS.md, documentation/**, per-crate README snippets,
CHANGELOGs (root, bin/bridge, every changed crate — \`just check-crate-changelogs\`),
per-crate .sqlx if SQL changed. Amend them into the release commit.

Then — every check below runs in the cloud:
  git push origin HEAD:next       # CI, Quality, Supply Chain run on the push
  just gate                       # read-only: the push run on the exact SHA must be green
  just promote <sha>              # refuses unless green; opens the release PR; merge it
  git fetch origin main && git checkout --detach origin/main
  git tag v${NEW_VERSION} bridge-v${NEW_VERSION} && git push origin v${NEW_VERSION} bridge-v${NEW_VERSION}
  for i in 1 2 3 4 5 6; do cargo ws publish --no-verify --publish-as-is --yes && break; sleep 20; done

Canonical flow: internal/release-flow.md
EOF
