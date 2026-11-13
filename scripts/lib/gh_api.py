"""Read-only GitHub REST access through `gh api`, shared by the release proof
and the flow report.

`get` retries a rate-limited call after the core limit resets; any other
failure raises `GhApiError` at once. `paginate` walks a list endpoint page by
page until a short page.
"""
import json
import subprocess
import time

PER_PAGE = 100
MAX_WAIT_SECONDS = 900


class GhApiError(RuntimeError):
    """A `gh api` call that failed for a reason other than a rate limit, or ran out of retries."""


def run_gh(*args):
    return subprocess.run(["gh", "api", *args], capture_output=True, text=True)


def rate_limit_reset():
    result = run_gh("rate_limit")
    if result.returncode != 0:
        raise GhApiError(f"gh api rate_limit: {result.stderr.strip()}")
    return json.loads(result.stdout)["resources"]["core"]["reset"]


def get(path, attempts=5, sleep=time.sleep, now=time.time):
    for attempt in range(attempts):
        result = run_gh(path)
        if result.returncode == 0:
            return json.loads(result.stdout)
        if "rate limit" not in result.stderr.lower() or attempt == attempts - 1:
            raise GhApiError(f"gh api {path}: {result.stderr.strip()}")
        sleep(max(1, min(MAX_WAIT_SECONDS, rate_limit_reset() - now() + 1)))
    raise GhApiError(f"gh api {path}: retries exhausted")


def with_page(path, page, per_page=PER_PAGE):
    separator = "&" if "?" in path else "?"
    return f"{path}{separator}per_page={per_page}&page={page}"


def paginate(path, key=None, fetch=get, per_page=PER_PAGE):
    """Every item of a list endpoint; `key` names the list inside an object body."""
    page = 1
    while True:
        body = fetch(with_page(path, page, per_page))
        items = body[key] if key else body
        yield from items
        if len(items) < per_page:
            return
        page += 1
