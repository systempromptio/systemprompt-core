#!/usr/bin/env python3
"""The base a gate run's affected selection is computed against.

BASE is the newest first-parent ancestor of the run's commit (the commit itself
excluded) on whose branch every gate workflow (`ci.yml`, `quality.yml`,
`supply-chain.yml`) has a successful push run. Push runs search the pushed
branch; `merge_group` runs search the queue's target branch, whose tip the
group commit descends from.

There is no base, and the run gates in full, when the event is not a push or
a merge group, when the push is a force-push or creates the branch, when the
GitHub API cannot be read, when the history is too shallow to walk, or when no
green commit lies within the walk. The reason is printed with the result.

    ci-base.py --event push --ref refs/heads/future --sha SHA --before SHA \\
               --forced false --repo OWNER/NAME [--output $GITHUB_OUTPUT]
    ci-base.py --event merge_group --base-ref refs/heads/next --sha SHA --repo OWNER/NAME
"""
import argparse
from itertools import islice
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))
import gh_api  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
GATE_WORKFLOWS = ("ci.yml", "quality.yml", "supply-chain.yml")
ZERO = "0" * 40
DEFAULT_LIMIT = 200


class BaseError(Exception):
    """The history or the run list could not be read."""


def green_shas(repo, branch, limit, fetch=gh_api.get):
    """Commits on `branch` with a successful push run of every gate workflow."""
    common = None
    for workflow in GATE_WORKFLOWS:
        path = f"repos/{repo}/actions/workflows/{workflow}/runs?branch={branch}&event=push&status=success"
        try:
            runs = list(islice(gh_api.paginate(path, "workflow_runs", fetch=fetch), limit))
        except gh_api.GhApiError as error:
            raise BaseError(str(error)) from error
        shas = {run["head_sha"] for run in runs
                if run.get("status") == "completed" and run.get("conclusion") == "success"
                and run.get("event") == "push" and run.get("head_branch") == branch}
        common = shas if common is None else common & shas
    return common or set()


def first_parents(sha, limit, git=None):
    """The first-parent ancestors of `sha`, nearest first, `sha` itself excluded."""
    git = git or (lambda *args: subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True))
    result = git("rev-list", "--first-parent", f"--max-count={limit + 1}", sha)
    if result.returncode != 0:
        raise BaseError(f"git rev-list {sha}: {result.stderr.strip()}")
    shallow = git("rev-parse", "--is-shallow-repository")
    if shallow.returncode == 0 and shallow.stdout.strip() == "true":
        raise BaseError("the checkout is shallow; fetch the full history (fetch-depth: 0)")
    return result.stdout.split()[1:]


def branch_of(ref):
    return ref.removeprefix("refs/heads/") if ref else ""


def resolve(args, fetch=gh_api.get, git=None):
    """`(base, reason)`: a green ancestor and why, or `("", why the run is full)`."""
    if args.event == "push":
        if args.forced:
            return "", "force-push: the previous tip is not an ancestor"
        if not args.before or args.before == ZERO:
            return "", "the push creates the branch"
        branch = branch_of(args.ref)
    elif args.event == "merge_group":
        branch = branch_of(args.base_ref)
    else:
        return "", f"{args.event} runs gate in full"
    if not branch:
        return "", f"no branch to search for a {args.event} run"
    try:
        green = green_shas(args.repo, branch, args.limit, fetch=fetch)
        for ancestor in first_parents(args.sha, args.limit, git=git):
            if ancestor in green:
                return ancestor, f"newest ancestor with green ci, quality and supply-chain push runs on {branch}"
    except BaseError as error:
        return "", f"cannot resolve a base: {error}"
    return "", f"no ancestor within {args.limit} first-parent commits has three green push runs on {branch}"


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--event", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--ref", default="")
    parser.add_argument("--before", default="")
    parser.add_argument("--forced", default="false", type=lambda text: text == "true")
    parser.add_argument("--base-ref", default="")
    parser.add_argument("--limit", type=int, default=DEFAULT_LIMIT)
    parser.add_argument("--output", help="append base= and base_reason= lines here (GITHUB_OUTPUT)")
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    base, reason = resolve(args)
    lines = f"base={base}\nbase_reason={reason}\n"
    print(lines, end="")
    if args.output:
        with open(args.output, "a") as handle:
            handle.write(lines)
    return 0


if __name__ == "__main__":
    sys.exit(main())
