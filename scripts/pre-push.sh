#!/usr/bin/env bash
# Opt-in pre-push hook (`just install-hooks`). Advisory only: it warns when a
# push to next fails the cheap gates (`scripts/gate.sh --changed --only
# fmt,gates`: format check and the source gates over the changed scope) or
# lands on a red tip, and always exits 0. No gate invokes it; the push runs on
# next remain the proof.

PUSHES_NEXT=0
while read -r _local_ref _local_sha remote_ref _remote_sha; do
  [[ "$remote_ref" == "refs/heads/next" ]] && PUSHES_NEXT=1
done
[[ "$PUSHES_NEXT" -eq 1 ]] || exit 0

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT" || exit 0

if ! bash scripts/gate.sh --changed --only fmt,gates >/dev/null 2>&1; then
  echo "pre-push warning: \`scripts/gate.sh --changed --only fmt,gates\` fails — this push will turn a gate red; run it to see which" >&2
fi

if command -v gh >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  if ! PROOF="$(python3 scripts/release-proof.py pushed origin/next 2>&1)"; then
    echo "pre-push warning: origin/next push run is not green: $(printf '%s\n' "$PROOF" | tail -n 1)" >&2
  fi
else
  echo "pre-push: origin/next push-run check skipped (gh, python3 or gh auth unavailable)" >&2
fi

exit 0
