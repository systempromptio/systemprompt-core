#!/usr/bin/env bash
# Choose the release tags the schema ladder upgrades from.
#
#   scripts/schema-ladder-rungs.sh
#
# Run from the checkout under test with its tags fetched. Prints a JSON list:
#
#   - the two newest release tags merged into HEAD (HEAD's own tag excluded),
#     the upgrade every deployed instance takes next;
#   - one skip rung: the newest release at least SKIP_MINORS minor versions
#     older than the newest rung, an instance that missed several releases and
#     upgrades straight to this tree.
#
# Every rung's binaries are cached by tag, so the skip rung costs a cold build
# once and a migrator-and-boot run after that.
set -euo pipefail

NEWEST_RUNGS=2
SKIP_MINORS=5

head_commit="$(git rev-parse HEAD)"
mapfile -t releases < <(
    git tag -l 'v[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname --merged HEAD \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
        | while read -r tag; do
              [ "$(git rev-parse "$tag^{commit}")" = "$head_commit" ] || echo "$tag"
          done
)

minor_key() {
    local version="${1#v}"
    local major="${version%%.*}" rest="${version#*.}"
    echo $(( major * 1000 + ${rest%%.*} ))
}

rungs=("${releases[@]:0:NEWEST_RUNGS}")
if [ "${#releases[@]}" -gt 0 ]; then
    ceiling=$(( $(minor_key "${releases[0]}") - SKIP_MINORS ))
    for tag in "${releases[@]}"; do
        if [ "$(minor_key "$tag")" -le "$ceiling" ]; then
            rungs+=("$tag")
            break
        fi
    done
fi

printf '%s\n' "${rungs[@]}" \
    | jq -Rsc 'reduce (split("\n")[] | select(length > 0)) as $t ([]; if any(.[]; . == $t) then . else . + [$t] end)'
