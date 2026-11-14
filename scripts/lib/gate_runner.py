"""The one gate runner behind `scripts/gate.sh`, identical locally and in CI.

Scope comes only from `scripts/affected.py`; this module turns a selection into
phases of exact commands, runs every step to completion, prints a summary and
appends one JSON line per run to the gate log.

Phases, in run order:

    db       `scripts/test-db.sh`, once, before test-workspace clippy and doc
             (`just doc-check` includes the test-workspace pass),
             which compile live against it (TEST_DATABASE_URL, SQLX_OFFLINE=false)
    fmt      `just format-check` (whole tree in every mode)
    clippy   root-workspace clippy over the affected crates, the bridge when it
             is affected, test-workspace clippy over the affected test crates
    doc      rustdoc over the same three sets (`just doc-check` in full mode)
    gates    the source gates of scripts/check-gates.txt: global gates always,
             unit gates with `--paths <affected crate dirs>`; every gate in full
             mode, then the rust-contracts tests (`just check-gates`).
             `--group G` keeps the gates scripts/gate-scopes.txt assigns to the
             quality.yml source-gates job G (the rust-contracts tests run with
             `errors`)
    supply   `just deny`, `just audit` and `scripts/ci-gitleaks.sh`, the three
             jobs of supply-chain.yml; gitleaks is skipped with a notice when
             no `gitleaks` binary is on PATH
    scripts  `scripts/test-scripts.sh` when a path under scripts/ changed
    tests    per affected shard group, `scripts/test-db.sh` then
             `scripts/test-shard.sh <group>`: a fresh migrated database per
             shard, exactly as `just test-shard` and CI run it

The tree hash in the log is `git write-tree` over a temporary index holding the
whole working tree (tracked, modified and untracked-not-ignored files), so two
runs over the same bytes share a hash whether or not anything was committed.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

import gate_scope

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
PHASES = ("fmt", "clippy", "doc", "gates", "supply", "scripts", "tests")
GROUPS = ("sql", "errors", "bridge", "release")
COSTS = ("cheap", "compile", "db")
RUST_CONTRACTS_GROUP = "errors"
UPSTREAMS = ("origin/future", "origin/next")
DEFAULT_LOG = ROOT / "target" / "gate-log" / "runs.jsonl"
SESSION_VARIABLES = ("CLAUDE_CODE_SESSION_ID", "GITHUB_RUN_ID")
RUST_CONTRACTS = ["cargo", "test", "--locked", "--manifest-path", "scripts/rust-contracts/Cargo.toml"]
CLIPPY_TAIL = ["--all-targets", "--all-features", "--keep-going", "--", "-D", "warnings"]
RUSTDOC_ENV = {"RUSTDOCFLAGS": "-D warnings"}
TEST_WORKSPACE = "crates/tests"


class GateError(Exception):
    """A gate invocation that cannot be planned: bad arguments, a failed selector or git call."""


def step(label, argv, env=None, cwd=".", db=None, skip=None):
    """One command; `db` marks a step that compiles or runs against the test database.

    A step with `skip` set is planned but not run: the reason is printed and the
    summary records it as skipped.
    """
    return {"label": label, "argv": list(argv), "env": dict(env or {}), "cwd": cwd,
            "db": cwd == TEST_WORKSPACE if db is None else db, "skip": skip}


def packages(names):
    return [arg for name in names for arg in ("-p", name)]


def full_selection(shard_groups):
    return {"mode": "full", "reasons": ["--full"], "clippy_packages": [], "test_packages": [],
            "affected_dirs": [], "test_dirs": [], "shard_groups": list(shard_groups), "bridge": True,
            "units": [], "scope_size": None}


def phase_fmt(selection, changed, group=None):
    return [step("format-check", ["just", "format-check"])]


def phase_clippy(selection, changed, group=None):
    if selection["mode"] == "full":
        return [step("clippy", ["just", "lint"]),
                step("clippy-tests", ["cargo", "clippy", "--workspace", *CLIPPY_TAIL], cwd=TEST_WORKSPACE)]
    steps = []
    if selection["clippy_packages"]:
        steps.append(step("clippy", ["cargo", "clippy", *packages(selection["clippy_packages"]), *CLIPPY_TAIL]))
    if selection["bridge"]:
        steps.append(step("clippy-bridge", ["just", "lint-bridge"]))
    if selection["test_packages"]:
        steps.append(step("clippy-tests", ["cargo", "clippy", *packages(selection["test_packages"]), *CLIPPY_TAIL],
                          cwd=TEST_WORKSPACE))
    return steps


def phase_doc(selection, changed, group=None):
    if selection["mode"] == "full":
        return [step("doc-check", ["just", "doc-check"], db=True)]
    steps = []
    if selection["clippy_packages"]:
        steps.append(step("doc", ["cargo", "doc", "--no-deps", "--all-features",
                                  *packages(selection["clippy_packages"])], RUSTDOC_ENV))
    if selection["bridge"]:
        steps.append(step("doc-bridge", ["cargo", "doc", "--manifest-path", "bin/bridge/Cargo.toml", "--no-deps"],
                          RUSTDOC_ENV))
    if selection["test_packages"]:
        steps.append(step("doc-tests", ["cargo", "doc", "--no-deps", *packages(selection["test_packages"])],
                          RUSTDOC_ENV, cwd=TEST_WORKSPACE))
    return steps


def phase_gates(selection, changed, group=None):
    table = gate_scope.gate_table()
    gates = [gate for gate in gate_scope.gate_list() if group is None or table[gate]["group"] == group]
    contracts = group in (None, RUST_CONTRACTS_GROUP)
    if selection["mode"] == "full":
        return [step(gate, ["just", gate]) for gate in gates] + \
            ([step("rust-contracts", RUST_CONTRACTS)] if contracts else [])
    paths = sorted(set(selection["affected_dirs"]) | set(selection["test_dirs"]))
    steps = []
    for gate in gates:
        if table[gate]["scope"] == "global":
            steps.append(step(gate, ["just", gate]))
        elif paths:
            steps.append(step(gate, ["just", gate, "--paths", *paths]))
    if contracts and any(path.startswith("scripts/rust-contracts/") for path in changed):
        steps.append(step("rust-contracts", RUST_CONTRACTS))
    return steps


def phase_supply(selection, changed, group=None, which=shutil.which):
    gitleaks = ["bash", "scripts/ci-gitleaks.sh"]
    absent = None if which("gitleaks") else "no gitleaks binary on PATH; supply-chain.yml runs it in CI"
    return [step("deny", ["just", "deny"]), step("audit", ["just", "audit"]),
            step("gitleaks", gitleaks, skip=absent)]


def phase_scripts(selection, changed, group=None):
    if selection["mode"] == "full" or any(path.startswith("scripts/") for path in changed):
        return [step("script-tests", ["bash", "scripts/test-scripts.sh"])]
    return []


def phase_tests(selection, changed, group=None):
    steps = []
    for group in selection["shard_groups"]:
        steps.append(step(f"test-db {group}", ["bash", "scripts/test-db.sh"], db=True))
        steps.append(step(f"shard {group}", ["bash", "scripts/test-shard.sh", group], db=True))
    return steps


BUILDERS = {"fmt": phase_fmt, "clippy": phase_clippy, "doc": phase_doc, "gates": phase_gates,
            "supply": phase_supply, "scripts": phase_scripts, "tests": phase_tests}


def needs_test_db(phases):
    return any(entry["db"] for _, steps in phases for entry in steps)


def needs_shared_db(phases):
    return any(entry["db"] for name, steps in phases if name != "tests" for entry in steps)


def gate_cost(label, table):
    """The scripts/gate-scopes.txt cost of a gates-phase step; the rust-contracts
    unit tests compile."""
    return table[label]["cost"] if label in table else "compile"


def plan(selection, changed, phases=PHASES, db_url=None, group=None, costs=None):
    """`[(phase, [step])]` in run order for the chosen phases; a phase may hold no steps.

    Steps that need the test database run live against `db_url`. When a step
    compiles in the test workspace, a `db` phase preparing it comes first; each
    test shard re-prepares it, so every shard starts fresh as in CI.
    """
    if group is not None and group not in GROUPS:
        raise GateError(f"unknown gate group {group} (groups: {', '.join(GROUPS)})")
    chosen = [(name, BUILDERS[name](selection, changed, group)) for name in PHASES if name in phases]
    if costs is not None:
        table = gate_scope.gate_table()
        chosen = [(name, [e for e in steps if gate_cost(e["label"], table) in costs] if name == "gates" else steps)
                  for name, steps in chosen]
    if not needs_test_db(chosen):
        return chosen
    if db_url is None:
        raise GateError("a test-workspace phase needs the test database URL")
    live = {"TEST_DATABASE_URL": db_url, "DATABASE_URL": db_url, "SQLX_OFFLINE": "false"}
    for _, steps in chosen:
        for entry in steps:
            if entry["db"]:
                entry["env"] = {**entry["env"], **live}
    if not needs_shared_db(chosen):
        return chosen
    return [("db", [step("test-db", ["bash", "scripts/test-db.sh"], {"TEST_DATABASE_URL": db_url})])] + chosen


def choose_costs(text):
    """The gate costs `--cost` keeps, or None (every gate) without it."""
    if not text:
        return None
    chosen = [c.strip() for c in text.split(",") if c.strip()]
    unknown = sorted(set(chosen) - set(COSTS))
    if unknown or not chosen:
        raise GateError(f"unknown gate cost(s): {', '.join(unknown) or text} (costs: {', '.join(COSTS)})")
    return set(chosen)


def choose_phases(only, skip):
    def names(text):
        chosen = [n.strip() for n in text.split(",") if n.strip()] if text else []
        unknown = sorted(set(chosen) - set(PHASES))
        if unknown:
            raise GateError(f"unknown phase(s): {', '.join(unknown)} (phases: {', '.join(PHASES)})")
        return chosen
    picked = names(only) or list(PHASES)
    dropped = set(names(skip))
    return [name for name in picked if name not in dropped]


def git(*args):
    result = subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True)
    if result.returncode != 0:
        raise GateError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout.strip()


def default_base():
    """Merge-base with the upstream the branch tracks, else the nearer of origin/future and origin/next."""
    try:
        upstream = git("rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}")
    except GateError:
        upstream = ""
    candidates = [upstream] if upstream in UPSTREAMS else [
        ref for ref in UPSTREAMS if subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--verify", "-q", ref],
                                                   capture_output=True).returncode == 0]
    if not candidates:
        raise GateError(f"no base: the branch tracks none of {', '.join(UPSTREAMS)}; pass --base")
    bases = [git("merge-base", "HEAD", ref) for ref in candidates]
    return min(bases, key=lambda base: int(git("rev-list", "--count", f"{base}..HEAD")))


def changed_since(base):
    """Committed, staged, unstaged and untracked-not-ignored paths differing from `base`."""
    tracked = git("diff", "--name-only", "--no-renames", base).splitlines()
    untracked = git("ls-files", "-o", "--exclude-standard").splitlines()
    return sorted(set(tracked) | set(untracked))


def run_selector(args, stdin=None):
    result = subprocess.run([sys.executable, "-B", str(SCRIPTS / "affected.py"), "--format", "json", *args],
                            input=stdin, capture_output=True, text=True)
    if result.returncode != 0:
        raise GateError(f"affected.py: {result.stderr.strip()}")
    return json.loads(result.stdout)


def all_shard_groups():
    result = subprocess.run(["bash", str(SCRIPTS / "test-shard.sh"), "--list"], capture_output=True, text=True)
    if result.returncode != 0:
        raise GateError(f"test-shard.sh --list: {result.stderr.strip()}")
    return result.stdout.split()


def test_db_url():
    result = subprocess.run(["bash", str(SCRIPTS / "test-db.sh"), "--print-url"], capture_output=True, text=True)
    if result.returncode != 0:
        raise GateError(f"test-db.sh --print-url: {result.stderr.strip()}")
    return result.stdout.strip()


def tree_hash():
    """`git write-tree` over the whole working tree, through a throwaway copy of the index."""
    index = git("rev-parse", "--path-format=absolute", "--git-path", "index")
    with tempfile.TemporaryDirectory() as scratch:
        temp = Path(scratch) / "index"
        if Path(index).exists():
            shutil.copyfile(index, temp)
        env = os.environ | {"GIT_INDEX_FILE": str(temp)}
        subprocess.run(["git", "-C", str(ROOT), "add", "-A"], env=env, check=True, capture_output=True)
        return subprocess.run(["git", "-C", str(ROOT), "write-tree"], env=env, check=True,
                              capture_output=True, text=True).stdout.strip()


def render_step(entry):
    env = " ".join(f"{key}={value!r}" for key, value in sorted(entry["env"].items()))
    if entry["skip"]:
        return f"# {entry['label']} skipped: {entry['skip']}"
    cd = f"(cd {entry['cwd']} && " if entry["cwd"] != "." else ""
    command = " ".join(entry["argv"])
    return f"{cd}{env + ' ' if env else ''}{command}{')' if cd else ''}"


def render_plan(header, phases):
    lines = list(header)
    for name, steps in phases:
        lines.append(f"[{name}]" + ("" if steps else " nothing in scope"))
        lines.extend(f"  {render_step(entry)}" for entry in steps)
    return "\n".join(lines)


def execute(phases):
    """Runs every step to completion; returns per-phase results with durations."""
    results = []
    for name, steps in phases:
        started = time.monotonic()
        outcomes = []
        for entry in steps:
            print(f"═══ {name}: {entry['label']} ═══", flush=True)
            if entry["skip"]:
                print(f"skipped: {entry['skip']}", flush=True)
                outcomes.append({"label": entry["label"], "result": "skip", "seconds": 0.0})
                continue
            began = time.monotonic()
            code = subprocess.run(entry["argv"], cwd=ROOT / entry["cwd"], env=os.environ | entry["env"]).returncode
            outcomes.append({"label": entry["label"], "result": "pass" if code == 0 else "fail",
                             "seconds": round(time.monotonic() - began, 1)})
        results_seen = {o["result"] for o in outcomes}
        status = "fail" if "fail" in results_seen else ("pass" if "pass" in results_seen else "skip")
        results.append({"phase": name, "result": status, "seconds": round(time.monotonic() - started, 1),
                        "steps": outcomes})
    return results


def summary(results):
    lines = ["", "═══ gate summary ═══"]
    for phase in results:
        lines.append(f"{phase['result'].upper():4}  {phase['phase']:8} {phase['seconds']:>8.1f}s")
        lines.extend(f"      {o['result'].upper():4}  {o['label']} ({o['seconds']:.1f}s)" for o in phase["steps"])
    failed = [f"{p['phase']}: {o['label']}" for p in results for o in p["steps"] if o["result"] == "fail"]
    lines.append(f"✗ {len(failed)} step(s) failed: {'; '.join(failed)}" if failed else "✓ every phase passed")
    return "\n".join(lines)


def log_record(*, mode, units, base, head, tree, selection, results, started, finished, environ, host):
    """One gate-log line. `result` is fail if any phase failed, else pass."""
    return {
        "timestamp": started.isoformat(),
        "mode": mode,
        "units": list(units),
        "base": base,
        "head": head,
        "tree": tree,
        "selector_mode": selection["mode"],
        "scope_size": selection["scope_size"],
        "counts": {key: len(selection[key]) for key in
                   ("clippy_packages", "test_packages", "affected_dirs", "test_dirs", "shard_groups")},
        "phases": [{"phase": p["phase"], "result": p["result"], "seconds": p["seconds"]} for p in results],
        "duration_seconds": round((finished - started).total_seconds(), 1),
        "result": "fail" if any(p["result"] == "fail" for p in results) else "pass",
        "host": host,
        "session": {name: environ[name] for name in SESSION_VARIABLES if environ.get(name)},
    }


def append_log(path, record):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")


def parse_args(argv):
    parser = argparse.ArgumentParser(prog="gate.sh", description="The one gate entry point.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--unit", nargs="+", metavar="CRATE", help="gate these crates and their reverse closure")
    mode.add_argument("--changed", action="store_true", help="gate what differs from --base, working tree included")
    mode.add_argument("--full", action="store_true", help="gate everything")
    parser.add_argument("--base", help="--changed base (default: merge-base with origin/future or origin/next)")
    parser.add_argument("--only", help=f"comma-separated phases to run ({','.join(PHASES)})")
    parser.add_argument("--skip", help="comma-separated phases to leave out")
    parser.add_argument("--group", choices=GROUPS, help="with the gates phase: only the gates of this quality.yml job")
    parser.add_argument("--cost", help=f"with the gates phase: only gates of these comma-separated costs ({','.join(COSTS)})")
    parser.add_argument("--selection", help="a precomputed `affected.py --format json` result to use as the scope")
    parser.add_argument("--dry-run", action="store_true", help="print the phases and exact commands, run nothing")
    args = parser.parse_args(argv)
    if args.base and not args.changed:
        parser.error("--base applies to --changed only")
    return args


def main(argv=None, environ=None):
    environ = os.environ if environ is None else environ
    args = parse_args(argv)
    try:
        phases_chosen = choose_phases(args.only, args.skip)
        costs = choose_costs(args.cost)
        base = None
        changed = []
        if args.changed:
            base = git("rev-parse", args.base) if args.base else default_base()
            changed = changed_since(base)
        if args.selection:
            selection = json.loads(Path(args.selection).read_text())
        elif args.full:
            selection = full_selection(all_shard_groups())
        elif args.unit:
            selection = run_selector(["--crates", *args.unit])
        else:
            selection = run_selector([], stdin="\n".join(changed) + "\n")
        if selection["mode"] == "full" and not args.full:
            selection = dict(full_selection(selection["shard_groups"] or all_shard_groups()),
                             reasons=selection["reasons"], scope_size=selection["scope_size"])
        mode = "full" if args.full else ("unit" if args.unit else "changed")
        draft = plan(selection, changed, phases_chosen, db_url="", group=args.group, costs=costs)
        phases = plan(selection, changed, phases_chosen, db_url=test_db_url() if needs_test_db(draft) else None,
                      group=args.group, costs=costs)
        head = git("rev-parse", "HEAD")
    except GateError as error:
        print(f"gate: {error}", file=sys.stderr)
        return 2
    header = [f"gate: {mode} (selector {selection['mode']}, scope {selection['scope_size']})"]
    header += [f"  base {base}"] if base else []
    header += [f"  unit {' '.join(args.unit)}"] if args.unit else []
    header += [f"  group {args.group}"] if args.group else []
    header += [f"  cost {args.cost}"] if args.cost else []
    header += [f"  full because {reason}" for reason in selection.get("reasons", []) if not args.full]
    print(render_plan(header, phases), flush=True)
    if args.dry_run:
        return 0
    started = datetime.now(timezone.utc)
    results = execute(phases)
    finished = datetime.now(timezone.utc)
    print(summary(results))
    record = log_record(mode=mode, units=args.unit or [], base=base, head=head, tree=tree_hash(),
                        selection=selection, results=results, started=started, finished=finished,
                        environ=environ, host=socket.gethostname())
    append_log(environ.get("GATE_LOG") or DEFAULT_LOG, record)
    return 1 if record["result"] == "fail" else 0


if __name__ == "__main__":
    sys.exit(main())
