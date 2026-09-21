#!/usr/bin/env bash
set -euo pipefail

search_root="${1:?usage: collect-fallback-profraw.sh <search-root> <run-marker> <output-dir> [prune-dir ...]}"
run_marker="${2:?usage: collect-fallback-profraw.sh <search-root> <run-marker> <output-dir> [prune-dir ...]}"
output_dir="${3:?usage: collect-fallback-profraw.sh <search-root> <run-marker> <output-dir> [prune-dir ...]}"
shift 3

test -d "$search_root"
test -f "$run_marker"
mkdir -p "$output_dir"

prune=("$search_root/target" "$@")
find_args=("$search_root" "(")
for path in "${prune[@]}"; do
    find_args+=(-path "$path" -o)
done
unset "find_args[$((${#find_args[@]} - 1))]"
find_args+=(")" -prune -o -type f -name 'default_*.profraw' -newer "$run_marker" -print0)

profile_list=$(mktemp)
trap 'rm -f "$profile_list"' EXIT
find "${find_args[@]}" > "$profile_list"

count=0
while IFS= read -r -d '' profile; do
    count=$((count + 1))
    destination="$output_dir/fallback-$count.profraw"
    test ! -e "$destination"
    cp -- "$profile" "$destination"
done < "$profile_list"

echo "$count"
