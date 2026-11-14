#!/usr/bin/env bash
# Gives a worktree of this repository its own cargo target directories and,
# when sccache is installed, compiles through it so worktrees share compiled
# dependencies.
#
#   scripts/worktree-cache.sh WORKTREE
#
# Each workspace keeps a target directory inside WORKTREE (`target/` and
# `crates/tests/target/`). A target directory shared between worktrees would
# hold cargo's exclusive build lock for a whole build, serialising every
# worktree behind the one compiling. sccache instead shares compiler outputs
# by content: registry dependencies resolve to the same paths in every
# worktree and hit the shared cache; workspace crates compile under each
# worktree's own absolute path, so they hit only within that worktree. CI
# compiles through the same tool.
#
# WORKTREE/.cargo/config.toml is the main checkout's untracked config without
# its [build] table; the tracked crates/tests/.cargo/config.toml gains a
# [build] table and is marked skip-worktree so the edit never reaches a commit.
# A config that already has a [build] table is left as it is, so a rerun
# changes nothing.
set -euo pipefail

[ "$#" -eq 1 ] || { echo "usage: $0 WORKTREE" >&2; exit 2; }
WT="$(cd "$1" && pwd)"
git -C "$WT" rev-parse --is-inside-work-tree >/dev/null

wrapper=""
command -v sccache >/dev/null && wrapper="sccache"

build_table() {
    printf '\n[build]\ntarget-dir = "%s"\n' "$1"
    [ -z "$wrapper" ] || printf 'rustc-wrapper = "%s"\n' "$wrapper"
}

mkdir -p "$WT/.cargo"
root_config="$WT/.cargo/config.toml"
if [ ! -f "$root_config" ] || ! grep -q '^\[build\]' "$root_config"; then
    main="$(cd "$(git -C "$WT" rev-parse --path-format=absolute --git-common-dir)/.." && pwd)"
    if [ -f "$main/.cargo/config.toml" ] && [ "$main" != "$WT" ]; then
        awk '/^\[build\]/{skip=1;next} /^\[/{skip=0} !skip' "$main/.cargo/config.toml" > "$root_config"
    fi
    build_table "$WT/target" >> "$root_config"
fi

tests_config="$WT/crates/tests/.cargo/config.toml"
if ! grep -q '^\[build\]' "$tests_config"; then
    build_table "$WT/crates/tests/target" >> "$tests_config"
    git -C "$WT" update-index --skip-worktree crates/tests/.cargo/config.toml
fi

echo "worktree-cache: $WT (target dirs per worktree; rustc wrapper: ${wrapper:-none, sccache not installed})"
