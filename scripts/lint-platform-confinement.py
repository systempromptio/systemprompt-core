#!/usr/bin/env python3
"""Keep target-system conditions inside the `platform` modules of the device crates.

A device crate (the DEVICE list of scripts/lint-device-closure.py) meets the
operating system through one `platform` module per concern: a single contract
and one real implementation per system in `platform/{windows,macos,linux}.rs`.
Everything outside those modules is the same code on every system, so a
missing implementation is a compile error on its target and never a stub that
answers at run time.

  1. No condition on the target system stands outside a `platform` module. A
     condition is `windows`, `unix`, `target_os`, `target_family`,
     `target_vendor` or `target_env` in the predicate of `#[cfg(..)]`,
     `#![cfg(..)]`, `#[cfg_attr(..)]` or `cfg!(..)`, and in a `cfg(..)` an
     attribute carries (`doc(cfg(..))`).
  2. A `platform` module is a module named `platform` and everything below
     it: `src/**/platform.rs`, `src/**/platform/**` and the braces of an
     inline `mod platform { .. }`. The attribute on the declaration of the
     module stands outside it.
  3. Every device crate has a manifest and a `src` under `crates/`, so a
     renamed or removed crate cannot leave the gate with nothing to judge.

Every Rust file of a device crate is read, its build script included.
Comments and string literals are not conditions; `cfg(test)`, features and
`target_arch` are not conditions on the target system. Findings are grouped
by crate, one line per attribute or macro.

Usage: lint-platform-confinement.py [--report] [ROOT]
  --report   print the findings and exit 0 (rule 3 still fails)
  ROOT       repository root to scan (default: this repository; fixtures pass one)
"""
import importlib.util
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS / "lib"))
from rust_tokens import located, rust_files  # noqa: E402

NAME = "lint-platform-confinement"
PLATFORM = "platform"
BARE = ("windows", "unix")
KEYED = ("target_os", "target_family", "target_vendor", "target_env")
PACKAGE_NAME = re.compile(r'^\[package\]\s*$.*?^name\s*=\s*"([^"]+)"', re.S | re.M)


def device_crates():
    """The device crate names, read from the closure gate so the list exists once."""
    spec = importlib.util.spec_from_file_location("lint_device_closure", SCRIPTS / "lint-device-closure.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.DEVICE


def crate_dirs(root):
    """`{package name: directory}` for every manifest at `crates/<layer>/<crate>`."""
    found = {}
    for manifest in sorted(root.glob("crates/*/*/Cargo.toml")):
        name = PACKAGE_NAME.search(manifest.read_text())
        if name:
            found[name.group(1)] = manifest.parent
    return found


def closing(tokens, i):
    """The index of the bracket that closes the `(` at `i`."""
    depth = 0
    for j in range(i, len(tokens)):
        if tokens[j][:2] == ("p", "("):
            depth += 1
        elif tokens[j][:2] == ("p", ")"):
            depth -= 1
            if depth == 0:
                return j
    return len(tokens) - 1


def predicate_end(tokens, i):
    """The index that ends the first argument of the `cfg_attr(` opened at `i`."""
    depth = 0
    for j in range(i, len(tokens)):
        kind, value, _ = tokens[j]
        if kind != "p":
            continue
        if value in "([{":
            depth += 1
        elif value in ")]}":
            depth -= 1
            if depth == 0:
                return j
        elif value == "," and depth == 1:
            return j
    return len(tokens) - 1


def named(tokens, start, end):
    """The target-system conditions among `tokens[start:end]`, in source order."""
    found = []
    for j in range(start, end):
        kind, value, _ = tokens[j]
        if kind != "id":
            continue
        if value in BARE:
            found.append(value)
        elif value in KEYED:
            keyed = j + 2 < end and tokens[j + 1][:2] == ("p", "=") and tokens[j + 2][0] == "str"
            found.append(f'{value} = "{tokens[j + 2][1]}"' if keyed else value)
    return list(dict.fromkeys(found))


def conditions(tokens):
    """`(line, names)` of each cfg outside an inline `mod platform { .. }`."""
    found, inline, braces = [], [], 0
    i, n = 0, len(tokens)
    while i < n:
        kind, value, line = tokens[i]
        before = tokens[i - 1][:2] if i else ("", "")
        opens = i + 1 < n and tokens[i + 1][:2] == ("p", "(")
        end = None
        if (kind, value) == ("p", "{"):
            braces += 1
        elif (kind, value) == ("p", "}"):
            if inline and inline[-1][1] == braces:
                inline.pop()
            braces -= 1
        elif (kind, value) == ("id", "mod"):
            if i + 2 < n and tokens[i + 1][0] == "id" and tokens[i + 2][:2] == ("p", "{"):
                inline.append((tokens[i + 1][1], braces + 1))
        elif (kind, value) == ("id", "cfg_attr") and opens and before == ("p", "["):
            end = predicate_end(tokens, i + 1)
        elif (kind, value) == ("id", "cfg") and opens and before not in (("p", "."), ("p", ":"), ("id", "fn")):
            end = closing(tokens, i + 1)
        elif (kind, value) == ("id", "cfg") and i + 2 < n and tokens[i + 1][:2] == ("p", "!") and tokens[i + 2][:2] == ("p", "("):
            end = closing(tokens, i + 2)
        if end is not None:
            names = named(tokens, i + 1, end)
            if names and not any(name == PLATFORM for name, _ in inline):
                found.append((line, names))
            i = end
            continue
        i += 1
    return found


def in_platform(rel):
    """Whether a crate-relative path is a file of a `platform` module."""
    parts = list(rel.parts)
    if parts[0] != "src":
        return False
    parts[-1] = parts[-1].removesuffix(".rs")
    return PLATFORM in parts[1:]


def judge(root, crate_dir):
    """`(findings, files judged)` for one device crate."""
    found, judged = [], 0
    for rel, path in rust_files(root, [crate_dir.relative_to(root).as_posix()]):
        judged += 1
        if in_platform(path.relative_to(crate_dir)):
            continue
        for line, names in conditions(located(path.read_text())):
            found.append(f"  {rel}:{line}: target-system condition ({', '.join(names)}) outside a `platform` module")
    return list(dict.fromkeys(found)), judged


def main(argv):
    args = list(argv)
    report = "--report" in args
    if report:
        args.remove("--report")
    if len(args) > 1 or any(arg.startswith("-") for arg in args):
        print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
        return 2
    root = Path(args[0]).resolve() if args else SCRIPTS.parent
    device = device_crates()
    dirs = crate_dirs(root)
    missing = [
        f"  {name}: a device crate has a manifest and a `src` under crates/"
        for name in device if name not in dirs or not (dirs[name] / "src").is_dir()
    ]
    total = judged = failing = 0
    for name in device:
        if name not in dirs:
            continue
        found, count = judge(root, dirs[name])
        judged += count
        if found:
            files = len({line.split(":")[0] for line in found})
            print(f"{name}: {len(found)} target-system condition(s) in {files} file(s)")
            print("\n".join(found))
            total += len(found)
            failing += 1
    if missing:
        print("\n".join(missing))
        print(f"{NAME}: FAIL ({len(missing)} of {len(device)} device crates not found)")
        return 1
    if total:
        verdict = "REPORT" if report else "FAIL"
        print(f"{NAME}: {verdict} ({total} target-system condition(s) outside a platform module in {failing} of {len(device)} device crates)")
        return 0 if report else 1
    print(f"{NAME}: OK ({judged} files of {len(device)} device crates hold target-system conditions only inside platform modules)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
