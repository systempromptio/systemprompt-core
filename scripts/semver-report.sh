#!/usr/bin/env bash
# Semver report: runs cargo-semver-checks over every published workspace crate
# against the last release tag and writes a markdown report. Informational —
# breaking changes are allowed, but each one reported here needs a
# `### Breaking` entry in CHANGELOG.md.
#
# Usage: scripts/semver-report.sh [OUTPUT.md]   (default: target/semver-report.md)
# BASELINE_REV overrides the detected `vX.Y.Z` tag.
set -uo pipefail
cd "$(dirname "$0")/.."

out="${1:-target/semver-report.md}"
mkdir -p "$(dirname "$out")"

baseline="${BASELINE_REV:-$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null)}"
[ -n "$baseline" ] || { echo 'No v* tag found; set BASELINE_REV' >&2; exit 2; }
command -v cargo-semver-checks >/dev/null || { echo 'cargo-semver-checks is not installed' >&2; exit 2; }

# Why: cargo metadata reports `publish = false` as an empty registry list.
mapfile -t unpublished < <(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.publish == []) | .name')
excludes=()
for package in "${unpublished[@]}"; do excludes+=(--exclude "$package"); done

# Why: cargo-semver-checks judges a change against the version bump, and on 0.x a
# minor bump already permits any break. Once the workspace version is ahead of
# the baseline tag, check as a patch release so every break is reported.
release_type=()
current="$(cargo metadata --no-deps --format-version 1 | jq -r '[.packages[] | select(.publish != [])][0].version')"
[ "$current" != "${baseline#v}" ] && release_type=(--release-type patch)

log="$(mktemp)"
trap 'rm -f "$log"' EXIT
cargo semver-checks check-release --workspace "${excludes[@]}" \
    --baseline-rev "$baseline" "${release_type[@]}" --color never >"$log" 2>&1
status=$?

failed="$(grep -cE '^\s*--- failure ' "$log" || true)"
unbuilt="$(grep -oE "failed to build rustdoc for crate [a-z0-9_-]+" "$log" | awk '{print $NF}' | sort -u | paste -sd' ' -)"
{
    echo "# Semver report against \`$baseline\`"
    echo
    if [ "$status" -eq 0 ]; then
        echo "No semver-breaking changes detected in published crates."
    elif [ "$failed" -eq 0 ]; then
        echo "**cargo-semver-checks failed without a finding (exit $status)**; the report is incomplete."
    else
        echo "**${failed} breaking finding(s)** (cargo-semver-checks exit $status)."
        echo "Every finding needs a \`### Breaking\` entry in CHANGELOG.md."
    fi
    if [ -n "$unbuilt" ]; then
        echo
        echo "**Not checked** (rustdoc failed): $unbuilt"
    fi
    echo
    echo '<details><summary>cargo-semver-checks output</summary>'
    echo
    echo '```text'
    grep -vE '^\s*(Building|Parsing|Checking|Checked|Compiling|Finished)\b' "$log"
    echo '```'
    echo '</details>'
} >"$out"

echo "Semver report written to $out (exit $status)"
exit "$status"
