#!/usr/bin/env bash
# The bridge test crates in scripts/bridge-native-crates.txt run in the native
# (Windows/macOS) quality matrix with no database and SQLX_OFFLINE=true. A
# dependency — direct or transitive within crates/tests — on
# systemprompt-test-fixtures, on any workspace crate that uses `sqlx::query*!`,
# or on linux-keyutils-keyring-store (Linux-only libc constants) cannot build
# there and only surfaces on the native runner. Fail here instead, in seconds.
# systemprompt-test-hack is refused the same way: it would pull the server
# stack into the native build, so crates/tests/.config/hakari.toml excludes
# exactly the listed crates from carrying it.
#
# The list is also complete: every systemprompt-bridge* package in crates/tests
# whose closure is clean must be listed, so quality.yml and coverage-bridge.yml
# exercise every bridge test crate that can run natively.
#
# Ownership of the "uses sqlx::query*!" fact is derived from the source tree
# of each crates/tests package, so a new query!-backed helper crate is caught
# without editing this script.
set -euo pipefail

cd "$(dirname "$0")/.."

mapfile -t native < <(grep -vE '^[[:space:]]*(#|$)' scripts/bridge-native-crates.txt)
if [ "${#native[@]}" -eq 0 ]; then
    echo "scripts/bridge-native-crates.txt lists no crates" >&2
    exit 1
fi

metadata=$(mktemp)
trap 'rm -f "$metadata"' EXIT
cargo metadata --manifest-path crates/tests/Cargo.toml --format-version 1 --no-deps --offline >"$metadata"

python3 - "$metadata" "${native[@]}" <<'PY'
import json, os, re, sys

with open(sys.argv[1], encoding="utf-8") as fh:
    meta = json.load(fh)
listed = sys.argv[2:]
pkgs = {p["name"]: p for p in meta["packages"]}
QUERY = re.compile(r"sqlx::query(_as|_scalar|_file|_file_as|_file_scalar)?!")
LINUX_ONLY = "linux-keyutils-keyring-store"
HACK = "systemprompt-test-hack"

query_cache = {}
def uses_query_macro(pkg):
    name = pkg["name"]
    if name not in query_cache:
        found = False
        src = os.path.join(os.path.dirname(pkg["manifest_path"]), "src")
        for dirpath, _, files in os.walk(src):
            for f in files:
                if f.endswith(".rs"):
                    with open(os.path.join(dirpath, f), encoding="utf-8", errors="ignore") as fh:
                        if QUERY.search(fh.read()):
                            found = True
                            break
            if found:
                break
        query_cache[name] = found
    return query_cache[name]

def linux_scoped(dep):
    target = dep.get("target") or ""
    return "linux" in target and "not" not in target

def offenders(root):
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
            elif name == HACK:
                bad.append(f"{name} (pins the server stack's features)")
            elif uses_query_macro(pkg):
                bad.append(f"{name} (uses sqlx::query*!)")
        for dep in pkg["dependencies"]:
            if dep.get("kind") not in (None, "dev"):
                continue
            if dep["name"] == LINUX_ONLY and not linux_scoped(dep):
                bad.append(f"{LINUX_ONLY} (Linux-only, via {name})")
            elif dep["name"] in pkgs:
                stack.append(dep["name"])
    return sorted(set(bad))

status = 0
for crate in listed:
    if crate not in pkgs:
        status = 1
        print(f"✗ {crate} is listed but is not a crates/tests package", file=sys.stderr)
        continue
    bad = offenders(crate)
    if bad:
        status = 1
        print(f"✗ {crate} runs in the native offline matrix but reaches:", file=sys.stderr)
        for line in bad:
            print(f"    {line}", file=sys.stderr)

tests_root = os.path.join(os.getcwd(), "crates", "tests") + os.sep
unlisted = sorted(
    name for name, p in pkgs.items()
    if name.startswith("systemprompt-bridge")
    and p["manifest_path"].startswith(tests_root)
    and name not in listed
    and not offenders(name)
)
for name in unlisted:
    status = 1
    print(f"✗ {name} is native-eligible but unlisted in scripts/bridge-native-crates.txt", file=sys.stderr)

with open(os.path.join("crates", "tests", ".config", "hakari.toml"), encoding="utf-8") as fh:
    excludes = re.search(r"^workspace-members = \[(.*?)^\]", fh.read(), re.S | re.M)
hack_free = set(re.findall(r'"([^"]+)"', excludes.group(1))) if excludes else set()
for name in sorted(hack_free ^ set(listed)):
    status = 1
    where = "scripts/bridge-native-crates.txt" if name in hack_free else "final-excludes.workspace-members"
    print(f"✗ {name} is missing from {where}; crates/tests/.config/hakari.toml must exclude exactly the native crates", file=sys.stderr)

if status:
    print(file=sys.stderr)
    print("Native bridge test crates must not depend on DB-backed fixtures, sqlx::query*! users", file=sys.stderr)
    print("or linux-keyutils-keyring-store, and every eligible bridge test crate must be listed.", file=sys.stderr)
    sys.exit(1)
print(f"✓ native bridge test crates are DB-free and complete ({len(listed)} crates)")
PY
