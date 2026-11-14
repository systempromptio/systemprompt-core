#!/usr/bin/env python3
"""The gate plan of one workflow run, derived from one affected selection.

Every job of ci.yml, quality.yml and supply-chain.yml has a rule in `JOBS`
saying when the selection needs it. The policy picks the plan a run executes:
`affected` (jobs and matrices follow the selection) only for pushes to a branch
in `AFFECTED_BRANCHES`, `full` everywhere else. Pushes to a branch in
`SHADOW_BRANCHES` execute the full plan and also run the shadow jobs, which
execute the scoped commands of the affected plan, so `escapes` can list what
the affected plan would have missed.

    ci-plan.py plan --workflow ci --event push --ref refs/heads/future \\
                    --selection selection.json --base SHA [--output $GITHUB_OUTPUT] \\
                    [--summary $GITHUB_STEP_SUMMARY]
    ci-plan.py aggregate --workflow ci --event push --ref refs/heads/future
                    (reads NEEDS = toJSON(needs) and SELECTION from the environment)
    ci-plan.py escapes --workflow ci --selection selection.json --jobs jobs.json \\
                    [--summary FILE] [--out escapes.json]

`aggregate` is the body of `CI passed` / `Quality passed` / `Supply Chain
passed`: a job passes when it succeeded, or when it was skipped and the rule
recomputed here from the selection says the plan did not need it. Anything else
(failure, cancellation, a skip the selection does not explain, a job missing
from the table) fails the aggregate.
"""
import argparse
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import re
import sys

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
sys.path.insert(0, str(SCRIPTS / "lib"))
import affected  # noqa: E402
import gate_runner  # noqa: E402

AFFECTED_BRANCHES = frozenset()
SHADOW_BRANCHES = frozenset({"future"})
# Where gate jobs download the selection artifact: under the ignored target/
# directory, so no gate that walks untracked files ever sees it.
SELECTION_FILE = "target/ci/selection.json"


@dataclass(frozen=True)
class Rule:
    """When a job runs, and how its display names map onto selection values.

    `kind` is `always`, `any` (the selection names any crate), or `fields`
    (any of `fields` is non-empty / true). A matrix job names its entries with
    `name` holding `{}`, or with `names` mapping matrix value to display name;
    `matrix_field` makes each entry's own run decision the value's membership in
    that selection list. `scoped` marks a job whose command the selection
    narrows (`-p` lists, `--paths`), and `shadow` the shadow job running that
    narrowed command on a shadow branch.
    """
    kind: str
    name: str
    fields: tuple = ()
    matrix_field: str = ""
    names: dict = field(default_factory=dict)
    scoped: bool = False
    shadow: str = ""


GATE_GROUP_NAMES = {
    "sql": "Source gates: schema / SQL / layers / headers",
    "errors": "Source gates: error handling / tasks / contracts",
    "bridge": "Source gates: bridge",
    "release": "Source gates: release / versions / gate parity",
}
SHADOW_GATES = "Shadow: scoped source gates"
SHADOW_CLIPPY = "Shadow: scoped clippy + rustdoc"

PLAN_JOBS = {
    "affected": Rule("always", "Affected selection"),
    "gate-plan": Rule("always", "Gate plan: {}"),
}
JOBS = {
    "ci": {
        **PLAN_JOBS,
        "fmt": Rule("always", "Format"),
        "build": Rule("any", "Build"),
        "build-no-engine": Rule("any", "Build (no document engine)"),
        "migrator": Rule("any", "Test migrator"),
        "test-tools": Rule("fields", "Test tools ({})", ("archive_sets",), matrix_field="archive_sets"),
        "test": Rule("fields", "Test ({})", ("shard_groups",), matrix_field="shard_groups"),
        "schema-ladder": Rule("any", "Schema ladder / {}"),
    },
    "quality": {
        **PLAN_JOBS,
        "bridge-web": Rule("fields", "Bridge web regression tests", ("bridge",)),
        "lint": Rule("fields", "Clippy lint baseline", ("clippy_packages", "bridge"), scoped=True,
                     shadow=SHADOW_CLIPPY),
        "lint-tests": Rule("fields", "Test workspace all-targets clippy + rustdoc", ("test_packages",), scoped=True),
        "bridge-native": Rule("fields", "Bridge native checks ({})", ("bridge",)),
        "docs": Rule("fields", "Rustdoc (warnings as errors)", ("clippy_packages", "bridge"), scoped=True,
                     shadow=SHADOW_CLIPPY),
        "source-gates": Rule("always", "", names=GATE_GROUP_NAMES, scoped=True, shadow=SHADOW_GATES),
        "msrv": Rule("fields", "MSRV check (Rust {})", ("clippy_packages", "bridge"), scoped=True),
        "file-size": Rule("always", "Source file size guard"),
    },
    "supply-chain": {
        "deny": Rule("always", "cargo deny"),
        "audit": Rule("always", "cargo audit"),
        "secrets": Rule("always", "gitleaks"),
    },
}
SHADOW_JOBS = {"ci": {}, "quality": {"shadow-gates": SHADOW_GATES, "shadow-clippy": SHADOW_CLIPPY}, "supply-chain": {}}
REPORT_JOBS = {"escapes": "Escape report"}


class PlanError(Exception):
    """A selection, needs document or job list that cannot be judged."""


def branch_of(ref):
    return (ref or "").removeprefix("refs/heads/")


def policy(event, ref):
    return "affected" if event == "push" and branch_of(ref) in AFFECTED_BRANCHES else "full"


def is_shadow(event, ref):
    return event == "push" and branch_of(ref) in SHADOW_BRANCHES


def needed(rule, selection):
    """Whether the affected plan of `selection` runs a job under `rule`."""
    if selection.get("mode") == "full" or rule.kind == "always":
        return True
    if selection.get("mode") not in ("affected", "none"):
        raise PlanError(f"selection mode {selection.get('mode')!r} is not full, affected or none")
    if rule.kind == "any":
        return selection["mode"] != "none"
    return any(bool(selection[name]) for name in rule.fields)


def decisions(workflow, selection, plan_mode):
    """`{job: runs}` for the plan a run executes."""
    jobs = table(workflow)
    if plan_mode == "full":
        return {job: True for job in jobs}
    return {job: needed(rule, selection) for job, rule in jobs.items()}


def table(workflow):
    if workflow not in JOBS:
        raise PlanError(f"unknown workflow {workflow} (known: {', '.join(JOBS)})")
    return JOBS[workflow]


def full_matrices(shard_map):
    groups = [group for group, _ in shard_map["groups"]]
    return groups, sorted(set(shard_map["archive_of"].values()))


def full_selection(shard_map, reasons):
    groups, sets = full_matrices(shard_map)
    return dict(gate_runner.full_selection(groups), archive_sets=sets, reasons=list(reasons))


PACKAGE_NAME = re.compile(r"^[A-Za-z0-9_-]+$")


def package_args(names, whole):
    """`--workspace`, or one `-p NAME` per package; the workflows expand it unquoted."""
    if whole:
        return "--workspace"
    bad = [name for name in names if not PACKAGE_NAME.match(name)]
    if bad:
        raise PlanError(f"package name(s) not usable as a cargo argument: {', '.join(bad)}")
    return " ".join(f"-p {name}" for name in names)


def gate_args(selection, base, whole):
    if whole or not base:
        return "--full"
    return f"--changed --base {base} --selection {SELECTION_FILE}"


def plan_outputs(workflow, selection, event, ref, base, shard_map):
    """Every `key=value` output the workflow's jobs read."""
    mode = policy(event, ref)
    whole = mode == "full" or selection["mode"] == "full"
    groups, sets = full_matrices(shard_map) if whole else (selection["shard_groups"], selection["archive_sets"])
    out = {
        "plan_mode": "full" if whole else "affected",
        "shadow": is_shadow(event, ref),
        "base": base,
        "selection": json.dumps(selection, separators=(",", ":")),
        "shard_groups": json.dumps(groups, separators=(",", ":")),
        "archive_sets": json.dumps(sets, separators=(",", ":")),
        "bridge": whole or selection["bridge"],
        "clippy_args": package_args(selection["clippy_packages"], whole),
        "tests_args": package_args(selection["test_packages"], whole),
        "gate_args": gate_args(selection, base, whole),
        "shadow_bridge": selection["mode"] == "full" or selection["bridge"],
        "shadow_clippy_args": package_args(selection["clippy_packages"], selection["mode"] == "full"),
        "shadow_gate_args": gate_args(selection, base, selection["mode"] == "full"),
    }
    for job, runs in decisions(workflow, selection, "full" if whole else "affected").items():
        out[f"run_{job.replace('-', '_')}"] = runs
    return out


def render_outputs(outputs):
    lines = []
    for key, value in outputs.items():
        value = ("true" if value else "false") if isinstance(value, bool) else str(value)
        if "\n" in value:
            raise PlanError(f"output {key} spans lines")
        lines.append(f"{key}={value}")
    return "\n".join(lines) + "\n"


def aggregate(workflow, needs, selection, event, ref):
    """`(ok, lines)`: the aggregate verdict over `needs` and the explanation."""
    jobs = table(workflow)
    missing = sorted(set(jobs) - set(needs))
    extra = sorted(set(needs) - set(jobs))
    lines = [f"{job}: listed in scripts/ci-plan.py but not in needs" for job in missing]
    lines += [f"{job}: in needs but has no rule in scripts/ci-plan.py" for job in extra]
    plan_mode = policy(event, ref)
    if "affected" in jobs:
        if needs.get("affected", {}).get("result") != "success":
            return False, lines + ["affected: the selection job did not succeed, so no skip can be explained"]
        if selection is None:
            return False, lines + ["no selection: the affected job published none"]
        if selection.get("mode") == "full":
            plan_mode = "full"
    runs = decisions(workflow, selection or {"mode": "full"}, plan_mode)
    ok = not missing and not extra
    for job in sorted(set(jobs) & set(needs)):
        result = needs[job].get("result")
        if result == "success":
            lines.append(f"{job}: success")
        elif result == "skipped" and not runs[job]:
            lines.append(f"{job}: skipped, the selection does not need it")
        else:
            ok = False
            reason = "the plan needs it" if result == "skipped" else "not success"
            lines.append(f"{job}: {result} ({reason})")
    return ok, lines


def entry_of(workflow, display):
    """`(job, matrix value or None)` for a job display name, or None for an unknown name."""
    for job, rule in table(workflow).items():
        for value, name in rule.names.items():
            if display == name:
                return job, value
        if not rule.name:
            continue
        pattern = "^" + re.escape(rule.name).replace(r"\{\}", "(.+)") + "$"
        match = re.match(pattern, display)
        if match:
            return job, match.group(1) if match.groups() else None
    return None


def escapes(workflow, selection, jobs):
    """Failed full-run jobs the affected plan of `selection` would have missed.

    `skipped`: the affected plan would not have run the job (or matrix entry).
    `scope`: it would have run the scoped command, and that command's shadow
    job passed. `unverified`: it would have run a scoped command no shadow job
    exercises, so whether the scope would have caught the failure is unknown.
    """
    rules = table(workflow)
    if selection.get("mode") == "full":
        return []
    by_name = {job["name"]: job.get("conclusion") for job in jobs}
    found = []
    for job in jobs:
        if job.get("conclusion") != "failure":
            continue
        entry = entry_of(workflow, job["name"])
        if entry is None:
            continue
        job_id, value = entry
        rule = rules[job_id]
        runs = needed(rule, selection)
        if runs and rule.matrix_field:
            runs = value in selection[rule.matrix_field]
        if not runs:
            found.append({"job": job["name"], "kind": "skipped"})
        elif rule.scoped and rule.shadow and by_name.get(rule.shadow) == "success":
            found.append({"job": job["name"], "kind": "scope", "shadow": rule.shadow})
        elif rule.scoped and not rule.shadow:
            found.append({"job": job["name"], "kind": "unverified"})
    return found


def escape_summary(workflow, selection, found):
    lines = [f"### Shadow escapes: {workflow}", "",
             f"Selection mode `{selection.get('mode')}`, scope {selection.get('scope_size')}."]
    lines += [f"- full because {reason}" for reason in selection.get("reasons", [])]
    if not found:
        lines += ["", "No escapes: every job red in the full run is one the affected plan runs unscoped."]
        return "\n".join(lines) + "\n"
    lines += ["", "| Job | Kind |", "|---|---|"]
    lines += [f"| {item['job']} | {item['kind']} |" for item in found]
    return "\n".join(lines) + "\n"


def kernel_lane_summary(selection):
    """The step-summary line reporting whether a selection mixes the kernel lane
    with capability units. Reported only; no plan decision reads it."""
    if "mixed_kernel_and_capability" not in selection:
        return "Kernel lane: not computed (no base, so no selection)\n"
    mixed = "true" if selection["mixed_kernel_and_capability"] else "false"
    kernel = ", ".join(selection.get("kernel_crates_touched", [])) or "none"
    capabilities = ", ".join(selection.get("capability_units_touched", [])) or "none"
    return (f"Kernel lane: mixed_kernel_and_capability: {mixed}"
            f" (kernel crates: {kernel}; capability units: {capabilities})\n")


def load_json(text, what):
    try:
        return json.loads(text)
    except (TypeError, json.JSONDecodeError) as error:
        raise PlanError(f"{what} is not JSON: {error}") from error


def main(argv=None, environ=None):
    environ = os.environ if environ is None else environ
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("plan", "aggregate", "escapes"):
        command = sub.add_parser(name)
        command.add_argument("--workflow", required=True, choices=sorted(JOBS))
        if name != "escapes":
            command.add_argument("--event", required=True)
            command.add_argument("--ref", default="")
    sub.choices["plan"].add_argument("--selection", required=True)
    sub.choices["plan"].add_argument("--base", default="")
    sub.choices["plan"].add_argument("--base-reason", default="")
    sub.choices["plan"].add_argument("--output")
    sub.choices["plan"].add_argument("--summary")
    sub.choices["escapes"].add_argument("--selection", required=True)
    sub.choices["escapes"].add_argument("--jobs", required=True)
    sub.choices["escapes"].add_argument("--summary")
    sub.choices["escapes"].add_argument("--out")
    args = parser.parse_args(argv)
    try:
        if args.command == "plan":
            return command_plan(args)
        if args.command == "aggregate":
            return command_aggregate(args, environ)
        return command_escapes(args)
    except (PlanError, affected.SelectorError, OSError) as error:
        print(f"ci-plan: {error}", file=sys.stderr)
        return 2


def command_plan(args):
    shard_map = affected.load_shard_map()
    path = Path(args.selection)
    if args.base:
        selection = load_json(path.read_text(), args.selection)
    else:
        selection = full_selection(shard_map, [args.base_reason or "no base"])
        path.write_text(json.dumps(selection, indent=2) + "\n")
    outputs = plan_outputs(args.workflow, selection, args.event, args.ref, args.base, shard_map)
    text = render_outputs(outputs)
    print(text, end="")
    if args.output:
        with open(args.output, "a") as handle:
            handle.write(text)
    lane = kernel_lane_summary(selection)
    print(lane, end="", file=sys.stderr)
    if args.summary:
        with open(args.summary, "a") as handle:
            handle.write(lane)
    return 0


def command_aggregate(args, environ):
    needs = load_json(environ.get("NEEDS"), "NEEDS")
    raw = environ.get("SELECTION") or ""
    selection = load_json(raw, "SELECTION") if raw else None
    ok, lines = aggregate(args.workflow, needs, selection, args.event, args.ref)
    print("\n".join(lines))
    if not ok:
        print(f"::error::a required {args.workflow} job did not succeed and the selection does not explain it")
    return 0 if ok else 1


def command_escapes(args):
    selection = load_json(Path(args.selection).read_text(), args.selection)
    jobs = load_json(Path(args.jobs).read_text(), args.jobs)
    found = escapes(args.workflow, selection, jobs)
    text = escape_summary(args.workflow, selection, found)
    print(text, end="")
    if args.summary:
        with open(args.summary, "a") as handle:
            handle.write(text)
    if args.out:
        Path(args.out).write_text(json.dumps({"workflow": args.workflow, "escapes": found}, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
