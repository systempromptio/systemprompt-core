#!/usr/bin/env bash
# Publish the tagged workspace to crates.io, retrying on 429, then verify every
# publishable crate's exact version on the sparse index.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

TAG=$(git describe --tags --exact-match --match 'v*' HEAD 2>/dev/null) \
    || { echo "HEAD is not a v* tag." >&2; exit 1; }
git fetch -q origin main --tags
git merge-base --is-ancestor HEAD origin/main \
    || { echo "$TAG is not on origin/main." >&2; exit 1; }
[ "$(git rev-parse "refs/tags/$TAG^{commit}")" = "$(git ls-remote origin "refs/tags/$TAG^{}" | cut -f1)" ] \
    || [ "$(git rev-parse "refs/tags/$TAG^{commit}")" = "$(git ls-remote origin "refs/tags/$TAG" | cut -f1)" ] \
    || { echo "$TAG on origin does not point at HEAD." >&2; exit 1; }

if [ -z "${CARGO_HTTP_CAINFO:-}" ] && [ -f /etc/ssl/certs/ca-certificates.crt ]; then
    export CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt
fi

CRATES=$(cargo metadata --format-version 1 --no-deps | jq -r '
    . as $m | .packages[] | select(.id as $id | $m.workspace_members | index($id))
    | select(.publish != []) | "\(.name) \(.version)"')
TOTAL=$(printf '%s\n' "$CRATES" | grep -c .)
echo "Publishing $TOTAL crates at $TAG"

LOG=$(mktemp); trap 'rm -f "$LOG"' EXIT
DELAY=60
for ATTEMPT in $(seq 1 10); do
    if cargo ws publish --no-verify --publish-as-is --yes 2>&1 | tee "$LOG"; then
        break
    fi
    if ! grep -qiE '429|too many requests|rate limit' "$LOG"; then
        echo "cargo ws publish failed without a rate limit; see above." >&2; exit 1
    fi
    [ "$ATTEMPT" -lt 10 ] || { echo "Still rate limited after $ATTEMPT attempts." >&2; exit 1; }
    echo "Rate limited (attempt $ATTEMPT); retrying in ${DELAY}s."
    sleep "$DELAY"
    DELAY=$(( DELAY * 2 > 600 ? 600 : DELAY * 2 ))
done

index_path() {
    local name=${1,,}
    case ${#name} in
        1) echo "1/$name" ;;
        2) echo "2/$name" ;;
        3) echo "3/${name:0:1}/$name" ;;
        *) echo "${name:0:2}/${name:2:2}/$name" ;;
    esac
}

FOUND=0
MISSING=()
while read -r NAME VERSION; do
    if curl -fsS "https://index.crates.io/$(index_path "$NAME")" \
        | jq -e --arg v "$VERSION" 'select(.vers == $v and .yanked == false)' >/dev/null 2>&1; then
        FOUND=$((FOUND + 1))
    else
        MISSING+=("$NAME@$VERSION")
    fi
done <<< "$CRATES"

echo "$FOUND/$TOTAL crates on the index"
if [ "${#MISSING[@]}" -gt 0 ]; then
    printf 'missing: %s\n' "${MISSING[@]}" >&2
    exit 1
fi
