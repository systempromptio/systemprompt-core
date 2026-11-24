#!/usr/bin/env python3
"""Keep the admission handoff's hidden constructors where they belong.

`Admitted::__from_facade` and `DeclaredCommands::__from_facade` build the
handoff the kernel mounts and dispatches from; only the facade
(`systemprompt/`) calls them. `GatewayPrincipal::__from_kernel` stamps a
verified gateway caller; only the API kernel's bridge credential check
(`crates/entry/api/src/routes/gateway/bridge_error.rs`) calls it, after the
gateway verifier accepted the bearer.
A call anywhere else in the root workspace's production source fails.

The facade also must not re-export a `systemprompt_api` or
`systemprompt_cli` function that takes an `Admitted` or a
`DeclaredCommands` (`run_composed`, `mount_admitted`, ...), so an install
reaches the kernel only through `Composition`. The banned set is read from
the `pub fn` signatures in those two crates. A `pub use` in the facade that
names one, globs (`*`) either crate or a module of it, or aliases either
crate whole, fails: a glob or alias re-exports every such function.

This gate binds core only; `crates/tests` (a separate workspace whose
acceptance tests forge handoffs on purpose) is not scanned. Installs are
bound by the kernel's re-check instead.

Usage: lint-hidden-constructors.py [ROOT]
  ROOT   repository root to scan (default: this repository; fixtures pass one)
"""
from pathlib import Path
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
from rust_tokens import located, rust_files  # noqa: E402

FACADE = "systemprompt"
ALLOWED = {
    "__from_facade": (FACADE,),
    "__from_kernel": ("crates/entry/api/src/routes/gateway/bridge_error.rs",),
}
SCANNED = ("crates", FACADE)
EXCLUDED = ("crates/tests",)
KERNEL_CRATES = {"systemprompt_api": "crates/entry/api/src", "systemprompt_cli": "crates/entry/cli/src"}
HANDOFF_TYPES = {"Admitted", "DeclaredCommands"}


def under(rel, prefixes):
    return any(rel == p or rel.startswith(p + "/") for p in prefixes)


def constructor_calls(root):
    found = []
    for rel, path in rust_files(root, SCANNED, EXCLUDED):
        tokens = located(path.read_text())
        for i, (kind, value, line) in enumerate(tokens):
            if kind != "id" or value not in ALLOWED:
                continue
            if i > 0 and tokens[i - 1][:2] == ("id", "fn"):
                continue
            if i + 1 < len(tokens) and tokens[i + 1][:2] == ("p", "(") and not under(rel, ALLOWED[value]):
                found.append(f"  {rel}:{line}: calls {value} outside {', '.join(ALLOWED[value])}")
    return found


def handoff_fns(root):
    banned = set()
    for src in KERNEL_CRATES.values():
        for _, path in rust_files(root, (src,)):
            tokens = located(path.read_text())
            for i in range(len(tokens) - 2):
                if tokens[i][:2] == ("id", "pub") and tokens[i + 1][:2] == ("id", "fn"):
                    name = tokens[i + 2][1]
                    j = i + 3
                    while j < len(tokens) and tokens[j][:2] not in (("p", "{"), ("p", ";")):
                        if tokens[j][0] == "id" and tokens[j][1] in HANDOFF_TYPES:
                            banned.add(name)
                        j += 1
    return banned


def facade_reexports(root, banned):
    found = []
    for rel, path in rust_files(root, (f"{FACADE}/src",)):
        tokens = located(path.read_text())
        i = 0
        while i < len(tokens) - 1:
            if tokens[i][:2] == ("id", "pub") and tokens[i + 1][:2] == ("id", "use"):
                j = i + 2
                while j < len(tokens) and tokens[j][:2] != ("p", ";"):
                    j += 1
                stmt = tokens[i + 2:j]
                if stmt and stmt[0][0] == "id" and stmt[0][1] in KERNEL_CRATES:
                    crate = stmt[0][1]
                    names = {t[1] for t in stmt if t[0] == "id"}
                    line = tokens[i][2]
                    if ("p", "*") in [t[:2] for t in stmt]:
                        found.append(f"  {rel}:{line}: glob re-export of {crate} reaches its handoff functions")
                    elif len(stmt) >= 3 and stmt[1][:2] == ("id", "as"):
                        found.append(f"  {rel}:{line}: crate alias of {crate} reaches its handoff functions")
                    for name in sorted(names & banned):
                        found.append(f"  {rel}:{line}: re-exports {crate} handoff function {name}")
                i = j
            i += 1
    return found


def main(argv):
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[1]
    found = constructor_calls(root) + facade_reexports(root, handoff_fns(root))
    if found:
        print("\n".join(found))
        print(f"lint-hidden-constructors: FAIL — {len(found)} finding(s)")
        return 1
    print("lint-hidden-constructors: OK — hidden constructors called only by their owners; no handoff function re-exported")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
