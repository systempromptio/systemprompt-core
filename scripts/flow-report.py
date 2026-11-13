#!/usr/bin/env python3
"""Flow report: lead time, gate share, rework, waste, collisions and churn for one branch.

The git-derived half reads local history only. The Actions-derived half lists the
branch's push runs through `gh api`, paginated in date slices, retried across
rate-limit resets and cached to a JSON file so a rerun refetches only the recent
slices. Every statistic is a pure function over plain dicts; I/O stays in the
`git_*`, `fetch_*` and `main` functions.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import sys
import time

SCRIPTS = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("release_proof", SCRIPTS / "release-proof.py")
release_proof = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(release_proof)

GATES = tuple(release_proof.AGGREGATES)
RUN_FIELDS = ("id", "name", "path", "head_sha", "event", "head_branch", "status", "conclusion",
              "run_attempt", "created_at", "run_started_at", "updated_at")
API_SEARCH_CAP = 1000
SETTLED = timedelta(days=1)


def parse_time(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def minutes(start, end):
    return (parse_time(end) - parse_time(start)).total_seconds() / 60


def percentile(values, q):
    ordered = sorted(values)
    if not ordered:
        return None
    rank = max(1, math.ceil(q * len(ordered)))
    return ordered[rank - 1]


def summary(values):
    values = list(values)
    return {"n": len(values), "median": percentile(values, 0.5), "p90": percentile(values, 0.9)}


def workflow_file(run):
    return run["path"].split("@", 1)[0].rsplit("/", 1)[-1]


def parse_git_log(text):
    commits = []
    for block in text.split("\x00"):
        lines = [line for line in block.splitlines() if line.strip()]
        if not lines:
            continue
        sha, stamp = lines[0].split("\t", 1)
        commits.append({"sha": sha, "time": stamp, "files": lines[1:]})
    return commits


def owning_crate(path, crate_dirs):
    best = None
    for directory in crate_dirs:
        if directory == "" or path == directory or path.startswith(directory + "/"):
            if best is None or len(directory) > len(best):
                best = directory
    return best


def crate_dirs_from_tree(paths):
    return sorted({path.rsplit("/", 1)[0] if "/" in path else "" for path in paths
                   if path == "Cargo.toml" or path.endswith("/Cargo.toml")})


def top_files(commits, n):
    counts = Counter(path for commit in commits for path in commit["files"])
    return sorted(counts.items(), key=lambda item: (-item[1], item[0]))[:n]


def crates_touched(commit, crate_dirs):
    return {crate for crate in (owning_crate(path, crate_dirs) for path in commit["files"]) if crate is not None}


def blast_radius(commits, crate_dirs):
    files = [len(commit["files"]) for commit in commits]
    crates = [len(crates_touched(commit, crate_dirs)) for commit in commits]
    histogram = Counter(min(count, 10) for count in crates)
    return {"files_per_commit": summary(files), "crates_per_commit": summary(crates),
            "crates_histogram": {("10+" if key == 10 else str(key)): histogram[key] for key in sorted(histogram)}}


def parse_kernel(text):
    return [line.strip() for line in text.splitlines() if line.strip() and not line.startswith("#")]


def touches_kernel(commit, prefixes):
    return any(path.startswith(prefix) for path in commit["files"] for prefix in prefixes)


def iso_week(stamp):
    year, week, _ = parse_time(stamp).isocalendar()
    return f"{year}-W{week:02d}"


def kernel_churn(commits, prefixes):
    weeks = defaultdict(lambda: [0, 0])
    for commit in commits:
        week = weeks[iso_week(commit["time"])]
        week[1] += 1
        if touches_kernel(commit, prefixes):
            week[0] += 1
    return [{"week": key, "kernel": value[0], "total": value[1]} for key, value in sorted(weeks.items())]


def release_lead_times(commits, releases):
    """`releases` is ordered oldest first: [{"tag", "time", "shas"}]; a commit's release is the first containing it."""
    first = {}
    for release in releases:
        for sha in release["shas"]:
            first.setdefault(sha, release)
    landed = [(commit, first.get(commit["sha"])) for commit in commits]
    hours = [minutes(commit["time"], release["time"]) / 60 for commit, release in landed if release]
    return {"hours": summary(hours), "unreleased": sum(1 for _, release in landed if release is None)}


def group_by_sha(runs, workflows=GATES):
    grouped = defaultdict(lambda: defaultdict(list))
    for run in runs:
        name = workflow_file(run)
        if name in workflows:
            grouped[run["head_sha"]][name].append(run)
    return grouped


def push_to_green(by_workflow, workflows=GATES):
    """Minutes from the first gate run's creation to the end of the last gate; None unless every gate's latest run is green."""
    latest = {}
    for name in workflows:
        runs = by_workflow.get(name)
        if not runs:
            return None
        latest[name] = max(runs, key=lambda run: run["id"])
        if latest[name]["status"] != "completed" or latest[name]["conclusion"] != "success":
            return None
    start = min(run["created_at"] for runs in by_workflow.values() for run in runs)
    end = max(run["updated_at"] for run in latest.values())
    return minutes(start, end)


def gate_minutes(by_workflow):
    return sum(minutes(run["created_at"], run["updated_at"]) for runs in by_workflow.values() for run in runs
               if run["status"] == "completed")


def lead_and_share(runs, workflows=GATES):
    grouped = group_by_sha(runs, workflows)
    green = {sha: push_to_green(by, workflows) for sha, by in grouped.items()}
    return {"pushed_shas": len(grouped),
            "green_shas": sum(1 for value in green.values() if value is not None),
            "push_to_green_minutes": summary(value for value in green.values() if value is not None),
            "gate_minutes_per_sha": summary(gate_minutes(by) for by in grouped.values())}


def rework(runs, workflows=GATES):
    grouped = group_by_sha(runs, workflows)
    red = [sum(1 for runs_ in by.values() for run in runs_ if run["conclusion"] == "failure") for by in grouped.values()]
    split = defaultdict(Counter)
    for run in runs:
        split[workflow_file(run)][run["conclusion"] or run["status"]] += 1
    return {"red_runs_per_sha": summary(red),
            "shas_with_red": sum(1 for count in red if count),
            "reruns": sum(1 for run in runs if run["run_attempt"] > 1),
            "conclusions": {name: dict(counter) for name, counter in sorted(split.items())}}


def waste(runs):
    per = Counter()
    for run in runs:
        if run["conclusion"] == "cancelled":
            per[workflow_file(run)] += minutes(run["run_started_at"], run["updated_at"])
    return {"per_workflow": {name: round(value, 1) for name, value in sorted(per.items())},
            "total": round(sum(per.values()), 1)}


def workflow_table(runs):
    grouped = defaultdict(list)
    for run in runs:
        grouped[workflow_file(run)].append(run)
    table = []
    for name, items in sorted(grouped.items()):
        done = [run for run in items if run["status"] == "completed" and run["conclusion"] in ("success", "failure")]
        ok = [run for run in done if run["conclusion"] == "success"]
        durations = summary(minutes(run["run_started_at"], run["updated_at"]) for run in ok)
        table.append({"workflow": name, "runs": len(items), "decided": len(done), "success": len(ok),
                      "success_rate": round(len(ok) / len(done), 3) if done else None,
                      "median_minutes": durations["median"], "p90_minutes": durations["p90"]})
    return table


def slices(since, until, step):
    """Slices on a grid anchored at the epoch, so cache keys stay stable as the window moves."""
    epoch = datetime(1970, 1, 1, tzinfo=timezone.utc)
    cursor = epoch + step * ((since - epoch) // step)
    while cursor < until:
        yield cursor, cursor + step
        cursor += step


def slim(run):
    return {field: run.get(field) for field in RUN_FIELDS}


def gh_json(path, attempts=5):
    for attempt in range(attempts):
        result = subprocess.run(["gh", "api", path], capture_output=True, text=True)
        if result.returncode == 0:
            return json.loads(result.stdout)
        if "rate limit" not in result.stderr.lower() or attempt == attempts - 1:
            raise RuntimeError(f"gh api {path}: {result.stderr.strip()}")
        reset = json.loads(release_proof.command("gh", "api", "rate_limit"))["resources"]["core"]["reset"]
        time.sleep(max(1, min(900, reset - time.time() + 1)))
    raise RuntimeError(f"gh api {path}: retries exhausted")


def fetch_slice(repo, branch, start, end):
    window = f"{start.strftime('%Y-%m-%dT%H:%M:%SZ')}..{end.strftime('%Y-%m-%dT%H:%M:%SZ')}"
    runs, page = [], 1
    while True:
        body = gh_json(f"repos/{repo}/actions/runs?branch={branch}&event=push&created={window}&per_page=100&page={page}")
        if page == 1 and body["total_count"] > API_SEARCH_CAP and end - start > timedelta(hours=1):
            middle = start + (end - start) / 2
            return fetch_slice(repo, branch, start, middle) + fetch_slice(repo, branch, middle, end)
        runs += [slim(run) for run in body["workflow_runs"]]
        if len(body["workflow_runs"]) < 100:
            return runs
        page += 1


def fetch_runs(repo, branch, since, until, cache_path, now):
    cache = json.loads(Path(cache_path).read_text()) if cache_path and Path(cache_path).exists() else {}
    runs = {}
    for start, end in slices(since, until, timedelta(days=3)):
        key = f"{repo}|{branch}|{start.isoformat()}|{end.isoformat()}"
        if key not in cache or end > now - SETTLED:
            cache[key] = fetch_slice(repo, branch, start, end)
        for run in cache[key]:
            if since <= parse_time(run["created_at"]) < until:
                runs[run["id"]] = run
    if cache_path:
        Path(cache_path).write_text(json.dumps(cache))
    return sorted(runs.values(), key=lambda run: run["id"])


def git(*args):
    return subprocess.check_output(["git", *args], text=True)


def git_commits(branch, since, until):
    text = git("log", branch, "--no-merges", f"--since={since.isoformat()}", f"--until={until.isoformat()}",
               "--format=%x00%H%x09%cI", "--name-only")
    return parse_git_log(text)


def git_package_manifests(branch):
    listing = git("grep", "-l", "^\\[package\\]", branch, "--", "Cargo.toml", "*/Cargo.toml").split()
    return [line.split(":", 1)[1] for line in listing]


def git_releases(branch):
    tags = git("for-each-ref", "--sort=creatordate", "--format=%(refname:short)\t%(creatordate:iso-strict)",
               "refs/tags/v[0-9]*.[0-9]*.[0-9]*").split("\n")
    releases, previous = [], None
    for line in filter(None, tags):
        tag, stamp = line.split("\t")
        span = f"{previous}..{tag}" if previous else tag
        releases.append({"tag": tag, "time": stamp, "shas": set(git("rev-list", span).split())})
        previous = tag
    return releases


def fmt(value, digits=1):
    return "–" if value is None else (f"{value:.{digits}f}" if isinstance(value, float) else str(value))


def render(report):
    out = [f"# Flow report: `{report['branch']}`", "",
           f"Window {report['since']} → {report['until']}; {report['commits']} non-merge commits "
           f"({report['first_sha'] or '–'}..{report['last_sha'] or '–'}).", ""]
    git_side = report["git"]
    out += ["## Lead time: landed → released (hours)", "", "| n | median | p90 | unreleased |", "|---|---|---|---|",
            f"| {git_side['landed_to_released']['hours']['n']} | {fmt(git_side['landed_to_released']['hours']['median'])} "
            f"| {fmt(git_side['landed_to_released']['hours']['p90'])} | {git_side['landed_to_released']['unreleased']} |", ""]
    out += ["## Collisions: most-committed files", "", "| File | Commits |", "|---|---|"]
    out += [f"| `{path}` | {count} |" for path, count in git_side["top_files"]]
    radius = git_side["blast_radius"]
    out += ["", "## Blast radius", "", "| Per commit | median | p90 |", "|---|---|---|",
            f"| files | {fmt(radius['files_per_commit']['median'])} | {fmt(radius['files_per_commit']['p90'])} |",
            f"| crates | {fmt(radius['crates_per_commit']['median'])} | {fmt(radius['crates_per_commit']['p90'])} |", "",
            "| Crates touched | Commits |", "|---|---|"]
    out += [f"| {key} | {value} |" for key, value in radius["crates_histogram"].items()]
    out += ["", "## Kernel churn", "", "| Week | Kernel commits | All commits |", "|---|---|---|"]
    out += [f"| {row['week']} | {row['kernel']} | {row['total']} |" for row in git_side["kernel_churn"]]
    actions = report.get("actions")
    if actions:
        lead = actions["lead"]
        out += ["", "## Lead time: push → green (gate workflows)", "",
                f"{lead['green_shas']} of {lead['pushed_shas']} pushed SHAs went green.", "",
                "| Measure (minutes) | n | median | p90 |", "|---|---|---|---|",
                f"| push → green | {lead['push_to_green_minutes']['n']} | {fmt(lead['push_to_green_minutes']['median'])} "
                f"| {fmt(lead['push_to_green_minutes']['p90'])} |",
                f"| gate queue+run per SHA | {lead['gate_minutes_per_sha']['n']} | {fmt(lead['gate_minutes_per_sha']['median'])} "
                f"| {fmt(lead['gate_minutes_per_sha']['p90'])} |"]
        work = actions["rework"]
        out += ["", "## Rework", "",
                f"Red gate runs per SHA: median {fmt(work['red_runs_per_sha']['median'])}, p90 {fmt(work['red_runs_per_sha']['p90'])}; "
                f"{work['shas_with_red']} SHAs saw a red gate; {work['reruns']} runs were re-run.", "",
                "| Workflow | Conclusions |", "|---|---|"]
        out += [f"| {name} | {', '.join(f'{key} {value}' for key, value in sorted(split.items()))} |"
                for name, split in work["conclusions"].items()]
        lost = actions["waste"]
        out += ["", f"## Waste: {fmt(lost['total'])} minutes in cancelled runs", "", "| Workflow | Minutes |", "|---|---|"]
        out += [f"| {name} | {fmt(value)} |" for name, value in lost["per_workflow"].items()]
        out += ["", "## Workflow durations (successful runs)", "",
                "| Workflow | Runs | Success rate | median min | p90 min |", "|---|---|---|---|---|"]
        out += [f"| {row['workflow']} | {row['runs']} | {fmt(row['success_rate'], 3)} | {fmt(row['median_minutes'])} "
                f"| {fmt(row['p90_minutes'])} |" for row in actions["workflows"]]
    return "\n".join(out) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--branch", default="next")
    parser.add_argument("--days", type=int, default=45)
    parser.add_argument("--since")
    parser.add_argument("--until")
    parser.add_argument("--top", type=int, default=20)
    parser.add_argument("--kernel", default=str(SCRIPTS / "flow-report-kernel.txt"))
    parser.add_argument("--no-actions", action="store_true")
    parser.add_argument("--cache")
    parser.add_argument("--repo")
    parser.add_argument("--out")
    parser.add_argument("--json")
    args = parser.parse_args(argv)

    now = datetime.now(timezone.utc).replace(microsecond=0)
    until = parse_time(args.until) if args.until else now
    since = parse_time(args.since) if args.since else until - timedelta(days=args.days)
    until, since = (value if value.tzinfo else value.replace(tzinfo=timezone.utc) for value in (until, since))

    commits = git_commits(args.branch, since, until)
    crate_dirs = crate_dirs_from_tree(git_package_manifests(args.branch))
    report = {"branch": args.branch, "since": since.isoformat(), "until": until.isoformat(), "commits": len(commits),
              "first_sha": commits[-1]["sha"][:9] if commits else None,
              "last_sha": commits[0]["sha"][:9] if commits else None,
              "git": {"top_files": top_files(commits, args.top),
                      "blast_radius": blast_radius(commits, crate_dirs),
                      "kernel_churn": kernel_churn(commits, parse_kernel(Path(args.kernel).read_text())),
                      "landed_to_released": release_lead_times(commits, git_releases(args.branch))}}
    if not args.no_actions:
        repo = args.repo or release_proof.repository()
        branch = args.branch.split("/", 1)[1] if args.branch.startswith("origin/") else args.branch
        runs = fetch_runs(repo, branch, since, until, args.cache, now)
        report["actions"] = {"runs": len(runs), "lead": lead_and_share(runs), "rework": rework(runs),
                             "waste": waste(runs), "workflows": workflow_table(runs)}

    markdown = render(report)
    if args.out:
        Path(args.out).write_text(markdown)
    else:
        sys.stdout.write(markdown)
    if args.json:
        Path(args.json).write_text(json.dumps(report, indent=2, default=sorted))


if __name__ == "__main__":
    main()
