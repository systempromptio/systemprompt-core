#!/usr/bin/env bash
# Scans the tracked tree for secrets with gitleaks and the repository's
# .gitleaks.toml, exactly as supply-chain.yml does: the tracked files are
# exported to a scratch directory first, so build output and untracked files
# are never scanned and history is not walked.
#
#   scripts/ci-gitleaks.sh            uses `gitleaks` on PATH
#   GITLEAKS=/path/to/gitleaks scripts/ci-gitleaks.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

gitleaks="${GITLEAKS:-gitleaks}"
command -v "$gitleaks" >/dev/null || { echo "ci-gitleaks: no $gitleaks binary" >&2; exit 2; }

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
git ls-files -z | tar --null --ignore-failed-read -T - -cf - | tar -xf - -C "$scratch"
cd "$scratch"
"$gitleaks" dir --no-banner --redact -c .gitleaks.toml .
