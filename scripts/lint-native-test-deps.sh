#!/usr/bin/env bash
# The bridge test crates in scripts/bridge-native-crates.txt run in the native
# (Windows/macOS) quality matrix with no database and SQLX_OFFLINE=true. A
# dependency — direct or transitive within crates/tests — on
# systemprompt-test-fixtures, or on any workspace crate that uses
# `sqlx::query*!`, drags a live-schema requirement into that matrix and only
# surfaces on the native runner. Fail here instead, in seconds.
#
# Ownership of the "uses sqlx::query*!" fact is derived from the source tree
# of each crates/tests package, so a new query!-backed helper crate is caught
# without editing this script.
set -euo pipefail

cd "$(dirname "$0")/.."

mapfile -t native < <(grep -vE '^\s*(#|$)' scripts/bridge-native-crates.txt)
if [ "${#native[@]}" -eq 0 ]; then
    echo "scripts/bridge-native-crates.txt lists no crates" >&2
    exit 1
fi

metadata=$(mktemp)
trap 'rm -f "$metadata"' EXIT
cargo metadata --manifest-path crates/tests/Cargo.toml --format-version 1 --no-deps >"$metadata"

status=0
for crate in "${native[@]}"; do
    # Transitive closure of workspace dependencies, names only, via the
    # resolved (no-deps) package list: workspace members reference each other
    # by name, so following `dependencies[].name` across members is exact.
    offenders=$(python3 - "$crate" "$metadata" <<'PY'
import json, os, re, sys

root = sys.argv[1]
with open(sys.argv[2], encoding="utf-8") as fh:
    meta = json.load(fh)
pkgs = {p["name"]: p for p in meta["packages"]}

def uses_query_macro(pkg):
    src = os.path.join(os.path.dirname(pkg["manifest_path"]), "src")
    if not os.path.isdir(src):
        return False
    for dirpath, _, files in os.walk(src):
        for f in files:
            if not f.endswith(".rs"):
                continue
            with open(os.path.join(dirpath, f), encoding="utf-8", errors="ignore") as fh:
                if re.search(r"sqlx::query(_as|_scalar|_file|_file_as|_file_scalar)?!", fh.read()):
                    return True
    return False

seen, stack, bad = set(), [root], []
while stack:
    name = stack.pop()
    if name in seen or name not in pkgs:
        continue
    seen.add(name)
    pkg = pkgs[name]
    if name != root:
        if name == "systemprompt-test-fixtures":
            bad.append(f"{name} (DB-backed fixtures)")
        elif uses_query_macro(pkg):
            bad.append(f"{name} (uses sqlx::query*!)")
    for dep in pkg["dependencies"]:
        if dep["name"] in pkgs and dep.get("kind") in (None, "dev"):
            stack.append(dep["name"])
print("\n".join(bad))
PY
)
    if [ -n "$offenders" ]; then
        status=1
        echo "✗ $crate runs in the native offline matrix but reaches:" >&2
        while IFS= read -r line; do echo "    $line" >&2; done <<<"$offenders"
    fi
done

if [ "$status" -ne 0 ]; then
    echo >&2
    echo "Native bridge test crates must not depend on DB-backed fixtures or sqlx::query*! users." >&2
    echo "Move the shared helper into a DB-free crate, or drop the dependency." >&2
    exit 1
fi
echo "✓ native bridge test crates are DB-free (${#native[@]} crates)"
