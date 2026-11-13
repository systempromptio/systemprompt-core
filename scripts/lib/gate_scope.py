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
"""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PATHS_FLAG = "--paths"
GATE_LIST = ROOT / "scripts" / "check-gates.txt"
GATE_TABLE = ROOT / "scripts" / "gate-scopes.txt"


class ScopeError(Exception):
    """A malformed `--paths` argument list."""


def gate_list(path=GATE_LIST):
    """The source gates of scripts/check-gates.txt, in run order."""
    return [line.strip() for line in Path(path).read_text().splitlines() if line.strip()]


def gate_table(path=GATE_TABLE):
    """`{gate: {"scope", "cost", "notes"}}` from scripts/gate-scopes.txt."""
    table = {}
    for line in Path(path).read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            fields = line.split(None, 3)
            table[fields[0]] = {"scope": fields[1], "cost": fields[2], "notes": fields[3] if len(fields) > 3 else ""}
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
