#!/usr/bin/env bash
set -euo pipefail

search_root="${1:?usage: quarantine-fallback-profraw.sh <search-root> <archive-root> [prune-dir ...]}"
archive_root="${2:?usage: quarantine-fallback-profraw.sh <search-root> <archive-root> [prune-dir ...]}"
shift 2

test -d "$search_root"
mkdir -p "$archive_root"

prune=("$search_root/target" "$@")
find_args=("$search_root" "(")
for path in "${prune[@]}"; do
    find_args+=(-path "$path" -o)
done
unset "find_args[$((${#find_args[@]} - 1))]"
find_args+=(")" -prune -o -type f -name 'default_*.profraw' -print0)

profile_list=$(mktemp)
trap 'rm -f "$profile_list"' EXIT
find "${find_args[@]}" > "$profile_list"

count=0
while IFS= read -r -d '' profile; do
    relative="${profile#"$search_root"/}"
    destination="$archive_root/$relative"
    mkdir -p "$(dirname "$destination")"
    test ! -e "$destination"
    mv -- "$profile" "$destination"
    count=$((count + 1))
done < "$profile_list"

echo "$count"
