#!/usr/bin/env bash
# Sync every `systemprompt-*` pin in [workspace.dependencies] to the current
# [workspace.package].version.
#
# Why this exists: `cargo ws version` silently skips crates it decides have
# had no changes since their last tag. The crates themselves still bump
# (they use `version.workspace = true`), but the pinned `version = "X.Y.Z"`
# strings in [workspace.dependencies] drift, breaking `cargo update` with
# errors like "candidate versions found which didn't match".
#
# Run this right after `cargo ws version ... --no-git-push --yes`.
set -euo pipefail

CARGO_TOML="${1:-Cargo.toml}"

if [[ ! -f "$CARGO_TOML" ]]; then
  echo "error: $CARGO_TOML not found" >&2
  exit 1
fi

VERSION="$(
  awk '
    /^\[workspace\.package\]/ { in_pkg = 1; next }
    /^\[/                     { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
      match($0, /"[^"]+"/)
      print substr($0, RSTART+1, RLENGTH-2)
      exit
    }
  ' "$CARGO_TOML"
)"

if [[ -z "$VERSION" ]]; then
  echo "error: could not find [workspace.package].version in $CARGO_TOML" >&2
  exit 1
fi

echo "Syncing workspace deps to version $VERSION"

# Rewrite every systemprompt-* entry in [workspace.dependencies] that pins
# a version string, regardless of the version it currently holds.
python3 - "$CARGO_TOML" "$VERSION" <<'PY'
import re, sys

path, version = sys.argv[1], sys.argv[2]
src = open(path).read()

pattern = re.compile(
    r'^(systemprompt(?:-[a-z-]+)?\s*=\s*\{[^}]*?version\s*=\s*")[^"]+(")',
    re.MULTILINE,
)
changed = 0
def repl(m):
    global changed
    changed += 1
    return f"{m.group(1)}{version}{m.group(2)}"

new = pattern.sub(repl, src)
if new != src:
    open(path, "w").write(new)
print(f"  updated {changed} dependency pin(s)")
PY

echo "Done. Run \`cargo update --workspace\` next."
