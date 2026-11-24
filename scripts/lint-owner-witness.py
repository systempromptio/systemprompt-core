#!/usr/bin/env python3
"""Refuse a read by id alone named from a user-policy handler module.

A route a member mounts with `AuthzPolicy::user()` serves one caller, so the
reads it makes are bounded by that caller: the repository takes the caller and
answers a foreign id exactly as an unknown one (`find_owned_run`,
`find_visible`, `owned_task`, `find_reported_by`). A read keyed by the id
alone is for CLI, job and admin code.

User-policy handler modules are found two ways:

* every `.route(Method::X, "path", module::handler, SPEC)` in a
  `RouteSet` declaration under `crates/entry/*/src` whose `SPEC` const is
  built from `AuthzPolicy::user()` (directly or through a helper fn in the
  same file); the handler resolves to the file under that crate's `src/`
  defining `fn handler`, preferring the one whose path names the module
  segments;
* the routers `crates/entry/api` mounts with `AuthzPolicy::user()` or
  `AuthzPolicy::authenticated()` through `declare`/`RouteProtection`
  (`USER_ROUTERS` below, each with the mount that makes it user-policy).

In those modules a finding is a call to a method in `ID_ONLY` or any name
ending in `AdminRepository`. A user-directory read (`find_by_id` on
`users`, `user_repository` or `user_provider()`) resolves the authenticated
principal itself and is not a finding. A handler that cannot be resolved, or a listed router
path that is gone, fails the gate (exit 2) rather than shrinking it.

Usage: lint-owner-witness.py [--root DIR] [--list-modules]
"""
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]

ID_ONLY = ("find_by_id", "get_by_id", "list_all", "find_run")

USER_ROUTERS = (
    ("crates/entry/files/src/http/issues", "documents.rs mount_files: issues under AuthzPolicy::user()"),
    ("crates/entry/files/src/http/documents", "documents.rs mount_files: user gate on documents and folders"),
    ("crates/entry/files/src/http/uploads", "documents.rs mount_files: user gate on uploads"),
    ("crates/entry/identity/src/http/oauth/client", "oauth_mount.rs: authenticated router under AuthzPolicy::user()"),
    ("crates/entry/identity/src/http/oauth/endpoints/introspect.rs", "oauth_mount.rs: authenticated router"),
    ("crates/entry/identity/src/http/oauth/endpoints/logout.rs", "oauth_mount.rs: authenticated router"),
    ("crates/entry/identity/src/http/oauth/endpoints/userinfo.rs", "oauth_mount.rs: authenticated router"),
    ("crates/entry/identity/src/http/oauth/endpoints/revoke.rs", "oauth_mount.rs: revocation router"),
    ("crates/entry/identity/src/http/users", "oauth_mount.rs mount_sessions: AuthzPolicy::user()"),
    ("crates/entry/agent/src/http/stream", "protocol/stream.rs: STREAM_BASE under AuthzPolicy::user()"),
    ("crates/entry/agent/src/http/webhook", "protocol/agent.rs: WEBHOOK under AuthzPolicy::authenticated()"),
    ("crates/entry/mcp/src/http/executions.rs", "protocol/mcp.rs: executions under AuthzPolicy::authenticated()"),
)

ROUTE = re.compile(
    r'\.route(?:_typed)?\(\s*Method::\w+\s*,\s*"[^"]*"\s*,\s*([A-Za-z_][\w:]*)\s*,\s*([A-Za-z_]\w*)'
)
SPEC = re.compile(r"const\s+([A-Z_][A-Z0-9_]*)\s*:\s*RouteSpec\s*=([^;]*);", re.S)
FN_BODY = re.compile(r"fn\s+(\w+)\s*\([^)]*\)[^{]*\{", re.S)
USER_POLICY = "AuthzPolicy::user()"
CALL = re.compile(r"\.(" + "|".join(ID_ONLY) + r")\s*\(")
ADMIN_TYPE = re.compile(r"\b\w+AdminRepository\b")
SELF_READ = re.compile(r"\b(?:users|user_repository|user_provider\(\))\s*$")


class Unresolved(Exception):
    pass


def strip_comment(line):
    in_str = False
    for i, ch in enumerate(line):
        if ch == '"':
            in_str = not in_str
        elif not in_str and line.startswith("//", i):
            return line[:i]
    return line


def user_helper_fns(text):
    names = set()
    for match in FN_BODY.finditer(text):
        depth, i = 1, match.end()
        while i < len(text) and depth:
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        if USER_POLICY in text[match.end():i]:
            names.add(match.group(1))
    return names


def user_specs(text):
    helpers = user_helper_fns(text)
    specs = set()
    for name, body in SPEC.findall(text):
        if USER_POLICY in body or any(re.search(rf"\b{h}\s*\(", body) for h in helpers):
            specs.add(name)
    return specs


def defining_files(src, name):
    pattern = re.compile(rf"\bfn\s+{re.escape(name)}\b")
    return sorted(p for p in src.rglob("*.rs") if pattern.search(p.read_text(errors="replace")))


def resolve(src, handler):
    *segments, name = handler.split("::")
    candidates = defining_files(src, name)
    named = [p for p in candidates if all(seg in p.relative_to(src).with_suffix("").parts for seg in segments)]
    picked = named or (candidates if len(candidates) == 1 else [])
    if len(picked) != 1:
        raise Unresolved(f"{src}: handler {handler} resolves to {len(picked)} file(s)")
    return picked[0]


def route_set_modules(root):
    modules = {}
    for decl in sorted((root / "crates" / "entry").glob("*/src/**/*.rs")):
        text = decl.read_text(errors="replace")
        if "RouteSet::" not in text:
            continue
        specs = user_specs(text)
        src = decl.parents[len(decl.relative_to(root).parts) - 4]
        for handler, spec in ROUTE.findall(text):
            if spec in specs:
                module = resolve(src, handler)
                modules.setdefault(module, f"{decl.relative_to(root)}: {handler} under {spec}")
    return modules


def router_modules(root):
    modules = {}
    for rel, why in USER_ROUTERS:
        path = root / rel
        if path.is_file():
            modules.setdefault(path, why)
        elif path.is_dir():
            for module in sorted(path.rglob("*.rs")):
                modules.setdefault(module, why)
        else:
            raise Unresolved(f"{rel}: listed user-policy router is gone")
    return modules


def user_modules(root, with_routers):
    modules = route_set_modules(root)
    if with_routers:
        for module, why in router_modules(root).items():
            modules.setdefault(module, why)
    return modules


def receiver(lines, number, column):
    text = lines[number][:column]
    while number > 0 and not re.search(r"[;{}=(]", text):
        number -= 1
        text = lines[number] + text
    return re.split(r"[;{}=(]", text)[-1].strip()


def findings(root, modules):
    found = []
    for module in sorted(modules):
        lines = [strip_comment(raw).strip() for raw in module.read_text(errors="replace").splitlines()]
        for index, line in enumerate(lines):
            where = f"{module.relative_to(root)}:{index + 1}"
            for match in CALL.finditer(line):
                if match.group(1) == "find_by_id" and SELF_READ.search(receiver(lines, index, match.start())):
                    continue
                found.append(f"  {where}: {match.group(1)} reads by id alone")
            for match in ADMIN_TYPE.finditer(line):
                found.append(f"  {where}: {match.group(0)} named from a user route")
    return found


def main(argv):
    root, list_only, with_routers = ROOT, False, True
    args = list(argv)
    while args:
        arg = args.pop(0)
        if arg == "--root" and args:
            root, with_routers = Path(args.pop(0)).resolve(), False
        elif arg == "--list-modules":
            list_only = True
        else:
            print(__doc__.split("Usage:")[1].strip(), file=sys.stderr)
            return 2
    try:
        modules = user_modules(root, with_routers)
    except Unresolved as error:
        print(f"lint-owner-witness: {error}", file=sys.stderr)
        return 2
    if list_only:
        for module in sorted(modules):
            print(f"{module.relative_to(root)}  ({modules[module]})")
        return 0
    found = findings(root, modules)
    if found:
        print("\n".join(found))
        print(f"lint-owner-witness: FAIL — {len(found)} id-only read(s) named from a user-policy handler")
        return 1
    print(f"lint-owner-witness: OK — {len(modules)} user-policy handler module(s) read through the caller")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
