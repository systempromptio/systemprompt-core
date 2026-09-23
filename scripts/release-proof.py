#!/usr/bin/env python3
"""Read-only candidate and promotion evidence for this repository."""
import argparse
import json
import re
import subprocess
import sys

WORKFLOWS = {"ci.yml": "CI passed", "quality.yml": "Quality passed", "supply-chain.yml": "Supply Chain passed"}


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def api(repo, path):
    return json.loads(command("gh", "api", f"repos/{repo}/{path}"))


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def repository():
    remote = command("git", "remote", "get-url", "origin")
    match = re.fullmatch(r"(?:https://github.com/|git@github.com:)([^/]+/[^/]+?)(?:\.git)?", remote)
    require(match, "origin must identify a GitHub repository")
    return match.group(1)


def validate_runs(runs, sha, number, repo, created_at):
    matching = [run for run in runs if run["head_sha"] == sha and run["event"] == "pull_request"
                and run.get("head_branch") == "promote"
                and run.get("head_repository", {}).get("full_name", "").lower() == repo.lower()
                and run["created_at"] >= created_at
                and (not run.get("pull_requests") or any(pr["number"] == number for pr in run["pull_requests"]))]
    require(matching, f"no promotion run for PR #{number} at {sha}")
    run = max(matching, key=lambda item: item["id"])
    require(run["status"] == "completed" and run["conclusion"] == "success",
            f"run {run['id']} is {run['status']}/{run['conclusion']}")
    return run


def validate_push_runs(runs, sha, repo):
    matching = [run for run in runs if run["head_sha"] == sha and run["event"] == "push"
                and run.get("head_branch") == "next"
                and run.get("head_repository", {}).get("full_name", "").lower() == repo.lower()]
    require(matching, f"no push run on next at {sha}; push it, or wait for the run to start")
    run = max(matching, key=lambda item: item["id"])
    require(run["status"] == "completed", f"push run {run['id']} is still {run['status']}")
    require(run["conclusion"] == "success", f"push run {run['id']} concluded {run['conclusion']}")
    return run


def verify_push(repo, sha):
    for workflow, aggregate in WORKFLOWS.items():
        runs = api(repo, f"actions/workflows/{workflow}/runs?event=push&head_sha={sha}&per_page=100")["workflow_runs"]
        run = validate_push_runs(runs, sha, repo)
        jobs = api(repo, f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100")["jobs"]
        require(any(job["name"] == aggregate and job["conclusion"] == "success" for job in jobs),
                f"push run {run['id']} lacks successful {aggregate}")
        print(f"{aggregate} on next: run {run['id']} attempt {run['run_attempt']}")


def verify_pr(repo, pr, sha):
    require(pr["head"]["sha"] == sha, "promotion head changed")
    require(pr["base"]["ref"] == "main" and pr["head"]["ref"] == "promote", "not a frozen promotion PR")
    require(pr["head"]["repo"]["full_name"].lower() == repo.lower(), "promotion belongs to another repository")
    number = pr["number"]
    tree = api(repo, f"git/commits/{sha}")["tree"]["sha"]
    merge_sha = pr.get("merge_commit_sha")
    require(merge_sha, "promotion merge commit is unavailable")
    require(api(repo, f"git/commits/{merge_sha}")["tree"]["sha"] == tree, "promotion merge changes the proven tree")
    sampled = {}
    for workflow, aggregate in WORKFLOWS.items():
        runs = api(repo, f"actions/workflows/{workflow}/runs?event=pull_request&head_sha={sha}&per_page=100")["workflow_runs"]
        run = validate_runs(runs, sha, number, repo, pr["created_at"])
        jobs = api(repo, f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100")["jobs"]
        require(any(job["name"] == aggregate and job["conclusion"] == "success" for job in jobs),
                f"run {run['id']} lacks successful {aggregate}")
        sampled[workflow] = (run["id"], run["run_attempt"])
        print(f"{aggregate}: run {run['id']} attempt {run['run_attempt']}")
    for workflow, identity in sampled.items():
        runs = api(repo, f"actions/workflows/{workflow}/runs?event=pull_request&head_sha={sha}&per_page=100")["workflow_runs"]
        run = validate_runs(runs, sha, number, repo, pr["created_at"])
        require((run["id"], run["run_attempt"]) == identity, "workflow proof changed while reading results")
    latest = api(repo, f"pulls/{number}")
    require(latest["head"]["sha"] == sha and latest["base"]["sha"] == pr["base"]["sha"], "promotion changed while reading proof")
    print(f"Promotion proof: PR #{number}, head {sha}, tree {tree}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["candidate", "pushed", "gate", "pr", "merged"])
    parser.add_argument("ref")
    args = parser.parse_args()
    sha = command("git", "rev-parse", f"{args.ref}^{{commit}}")
    repo = repository()
    if args.mode == "merged":
        prs = api(repo, f"commits/{sha}/pulls?per_page=100")
        matches = [pr for pr in prs if pr.get("merged_at") and pr.get("merge_commit_sha") == sha
                   and pr["base"]["ref"] == "main" and pr["head"]["ref"] == "promote"]
        require(len(matches) == 1, "release commit must be the merge of one frozen promotion PR")
        pr = api(repo, f"pulls/{matches[0]['number']}")
        verify_pr(repo, pr, pr["head"]["sha"])
        return
    next_sha = api(repo, "git/ref/heads/next")["object"]["sha"]
    main_sha = api(repo, "git/ref/heads/main")["object"]["sha"]
    require(api(repo, f"compare/{sha}...{next_sha}")["status"] in ("ahead", "identical"), "candidate is not on origin/next")
    require(api(repo, f"compare/{main_sha}...{sha}")["status"] in ("ahead", "identical"), "candidate does not contain current main")
    if args.mode == "candidate":
        print(f"Candidate {sha} is on next and contains main; release proof runs on its promotion PR")
        return
    if args.mode == "pushed":
        verify_push(repo, sha)
        print(f"Candidate {sha} is on next, contains main and is green on its push run")
        return
    if args.mode == "gate":
        verify_push(repo, sha)
    prs = api(repo, "pulls?state=open&base=main&head=" + repo.split('/')[0] + ":promote&per_page=100")
    matches = [pr for pr in prs if pr["head"]["sha"] == sha]
    if not matches and args.mode == "gate":
        print(f"Candidate {sha} is green on next and ready to promote. Run just promote {sha}.")
        return
    require(len(matches) == 1, "no unique open promotion PR for this exact candidate")
    verify_pr(repo, api(repo, f"pulls/{matches[0]['number']}"), sha)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, KeyError) as error:
        print(f"release-proof: {error}", file=sys.stderr)
        sys.exit(1)
