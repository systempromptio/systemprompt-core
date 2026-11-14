#!/usr/bin/env python3
"""The path scope every unit source gate shares.

A unit gate takes an optional trailing `--paths P [P ...]`. Without it the gate
runs in full mode: the whole repository, stale-allowlist checks included,
exactly as `just check-gates` runs it. With it the gate examines only files at
or under the named paths (files or directories, relative to the repository
root or absolute inside it) and skips every check that needs the whole tree to
decide, such as an allowlist entry no file uses any more.

Files are enumerated as tracked plus untracked-not-ignored
(`git ls-files -co --exclude-standard`), restricted to files that exist, so a
gate run before `git add` sees a new file.

Used as a module by the Python gates and as a command by `gate-scope.sh`:

    gate_scope.py files  SPEC... [--paths P...]   matching files, root-relative
    gate_scope.py roots  ROOT... [--paths P...]   ROOTs narrowed to the scope
    gate_scope.py touching DIR... [--paths P...]  the DIRs that overlap the scope
    gate_scope.py filter [--paths P...]           stdin `path[:rest]` lines in scope
    gate_scope.py allowlist GATE                  GATE's allowlist entries, root-relative
    gate_scope.py check-allowlists                every per-unit allowlist file is well placed

Allowlists are per unit. A package declares its entries for gate GATE in
`<package dir>/gates/GATE.allow`, one entry per line, the leading path relative
to the package directory; `#` starts a comment line. An entry that lies under no
package (a test-workspace tree) stays in the gate's central file in
`CENTRAL_ALLOWLISTS`. `allowlist` returns the central entries and every
package's entries with the leading path rebased onto the repository root, so a
gate judges one root-relative list whichever file an entry came from.
"""
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PATHS_FLAG = "--paths"
GATE_LIST = ROOT / "scripts" / "check-gates.txt"
GATE_TABLE = ROOT / "scripts" / "gate-scopes.txt"


ALLOWLIST_DIR = "gates"
ALLOWLIST_SUFFIX = ".allow"
ALLOWLIST_GATES = (
    "lint-env-vars", "lint-sqlx", "lint-repo-construction", "lint-serde-aliases",
    "lint-core-globals", "check-no-vendor", "lint-table-ownership", "lint-ledger-reads",
)
CENTRAL_ALLOWLISTS = {
    "lint-sqlx": "scripts/sqlx-allowlist.txt",
    "check-no-vendor": "scripts/core-markup-allowlist.txt",
}
LEADING_PATH = re.compile(r"^([^\s:|#]+)(.*)$")


class ScopeError(Exception):
    """A malformed `--paths` argument list."""


class AllowlistError(Exception):
    """A misplaced per-unit allowlist file or a malformed entry."""


def gate_list(path=GATE_LIST):
    """The source gates of scripts/check-gates.txt, in run order."""
    return [line.strip() for line in Path(path).read_text().splitlines() if line.strip()]


def gate_table(path=GATE_TABLE):
    """`{gate: {"scope", "cost", "group", "notes"}}` from scripts/gate-scopes.txt."""
    table = {}
    for line in Path(path).read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            fields = line.split(None, 4)
            table[fields[0]] = {"scope": fields[1], "cost": fields[2], "group": fields[3],
                                "notes": fields[4] if len(fields) > 4 else ""}
    return table


def parse(argv):
    """Split argv into (gate arguments, scope); scope is None in full mode."""
    argv = list(argv)
    if PATHS_FLAG not in argv:
        return argv, None
    at = argv.index(PATHS_FLAG)
    paths = argv[at + 1:]
    if not paths:
        raise ScopeError(f"{PATHS_FLAG} needs at least one path")
    return argv[:at], [normalise(p) for p in paths]


def normalise(path):
    """A scope path relative to the repository root, without `./` or a trailing `/`."""
    path = str(path)
    if os.path.isabs(path):
        resolved = os.path.normpath(path)
        root = str(ROOT)
        if resolved == root:
            return ""
        if resolved.startswith(root + os.sep):
            return resolved[len(root) + 1:]
        raise ScopeError(f"{path} is outside the repository")
    path = os.path.normpath(path)
    return "" if path == "." else path


def relative(path):
    """`path` relative to the repository root when it lies inside it."""
    path = str(path)
    root = str(ROOT) + os.sep
    if path.startswith(root):
        return path[len(root):]
    return path[2:] if path.startswith("./") else path


def in_scope(path, scope):
    if scope is None:
        return True
    path = relative(path).rstrip("/")
    return any(s == "" or path == s or path.startswith(s + "/") for s in scope)


def files(pathspecs, scope=None):
    """Tracked and untracked-not-ignored files matching `pathspecs`, in scope."""
    listed = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-co", "--exclude-standard", "--", *pathspecs],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    return sorted(
        {f for f in listed if in_scope(f, scope) and (ROOT / f).is_file()}
    )


def narrowed(base, s):
    """The part of `base` the scope path `s` covers, or None when they are disjoint."""
    if s == "" or base == s or base.startswith(s + "/"):
        return base
    if s.startswith(base + "/"):
        return s
    return None


def roots(defaults, scope):
    """The default roots narrowed to the scope, keeping each default's spelling."""
    if scope is None:
        return list(defaults)
    out = []
    for default in defaults:
        absolute = os.path.isabs(default)
        base = relative(default).rstrip("/")
        for s in scope:
            pick = narrowed(base, s)
            if pick is None:
                continue
            spelled = str(ROOT / pick) if absolute else pick
            if spelled not in out and (ROOT / pick).exists():
                out.append(spelled)
    return out


def touching(defaults, scope):
    """The defaults, unnarrowed, that overlap the scope at all."""
    if scope is None:
        return list(defaults)
    return [
        d for d in defaults
        if any(narrowed(relative(d).rstrip("/"), s) is not None for s in scope)
    ]


def is_package_dir(directory):
    manifest = ROOT / directory / "Cargo.toml"
    return manifest.is_file() and re.search(r"(?m)^\[package\]", manifest.read_text()) is not None


def owning_package(path, packages):
    """The deepest package directory in `packages` holding `path`, or None."""
    best = None
    for directory in packages:
        if path == directory or path.startswith(directory + "/"):
            if best is None or len(directory) > len(best):
                best = directory
    return best


def package_dirs():
    """Every tracked package directory except the repository root."""
    manifests = files(["Cargo.toml", "*/Cargo.toml"])
    return sorted(d for d in (str(Path(m).parent) for m in manifests) if d != "." and is_package_dir(d))


def allowlist_files(gate=None):
    """`{file: package dir}` for every per-unit allowlist (of `gate`, or of any gate)."""
    name = f"{gate}{ALLOWLIST_SUFFIX}" if gate else f"*{ALLOWLIST_SUFFIX}"
    found = {}
    for f in files([f"*/{ALLOWLIST_DIR}/{name}"]):
        path = Path(f)
        if path.parent.name != ALLOWLIST_DIR:
            continue
        package = str(path.parent.parent)
        if not is_package_dir(package):
            raise AllowlistError(f"{f}: {ALLOWLIST_DIR}/ sits beside no package Cargo.toml")
        if path.stem not in ALLOWLIST_GATES:
            raise AllowlistError(f"{f}: no gate reads per-unit allowlists named {path.name}"
                                 f" (one of: {', '.join(ALLOWLIST_GATES)})")
        found[f] = package
    return found


def entry_lines(path):
    for raw in (ROOT / path).read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line and not line.startswith("#"):
            yield line


def rebase(line, package, source):
    match = LEADING_PATH.match(line)
    if match is None:
        raise AllowlistError(f"{source}: '{line}' does not start with a path")
    path, rest = match.groups()
    if path.startswith("/") or ".." in Path(path).parts:
        raise AllowlistError(f"{source}: '{path}' leaves its package; write it relative to {package}")
    return f"{package}/{path}{rest}"


def allowlist(gate):
    """GATE's entries: the central file's, then each package's, rebased onto the root."""
    if gate not in ALLOWLIST_GATES:
        raise AllowlistError(f"{gate} reads no per-unit allowlist")
    out = []
    central = CENTRAL_ALLOWLISTS.get(gate)
    if central:
        out.extend(entry_lines(central))
    for source, package in sorted(allowlist_files(gate).items()):
        out.extend(rebase(line, package, source) for line in entry_lines(source))
    return out


def check_allowlists():
    """Problems with the per-unit layout: misplaced files, central entries a package owns."""
    problems = []
    try:
        allowlist_files()
    except AllowlistError as error:
        problems.append(str(error))
    packages = package_dirs()
    for gate, central in CENTRAL_ALLOWLISTS.items():
        for line in entry_lines(central):
            match = LEADING_PATH.match(line)
            owner = owning_package(match.group(1).rstrip("/").replace("*", "_"), packages) if match else None
            if owner:
                problems.append(f"{central}: '{line}' lies in package {owner};"
                                f" it belongs in {owner}/{ALLOWLIST_DIR}/{gate}{ALLOWLIST_SUFFIX}")
    return problems


def main(argv):
    if not argv:
        print(__doc__, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    try:
        args, scope = parse(rest)
    except ScopeError as error:
        print(f"gate-scope: {error}", file=sys.stderr)
        return 2
    listers = {"files": files, "roots": roots, "touching": touching}
    if command in listers:
        found = listers[command](args, scope)
        if found:
            print("\n".join(found))
    elif command == "allowlist":
        if len(args) != 1:
            print("gate-scope: allowlist takes one GATE", file=sys.stderr)
            return 2
        try:
            entries = allowlist(args[0])
        except AllowlistError as error:
            print(f"gate-scope: {error}", file=sys.stderr)
            return 2
        if entries:
            print("\n".join(entries))
    elif command == "check-allowlists":
        problems = check_allowlists()
        for problem in problems:
            print(f"gate-scope: {problem}", file=sys.stderr)
        return 1 if problems else 0
    elif command == "filter":
        for line in sys.stdin:
            if in_scope(line.split(":", 1)[0].rstrip("\n"), scope):
                sys.stdout.write(line)
    else:
        print(f"gate-scope: unknown command {command}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
