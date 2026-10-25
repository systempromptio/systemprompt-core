#!/usr/bin/env bash
# Lay the coverage build artifacts out at the build job's target directory.
#
# The binaries metadata recorded by the build job names absolute paths under
# its CARGO_TARGET_DIR. Unpacking the nextest archive's `target/` tree at that
# same path lets a shard or report runner use the metadata unchanged, so the
# shard script's metadata-reuse path runs the archive without invoking cargo.
#
# Usage: scripts/coverage-unpack.sh <tests-artifact-dir> <bins-artifact-dir> <target-dir>
set -euo pipefail

tests_dir="${1:?usage: coverage-unpack.sh <tests-artifact-dir> <bins-artifact-dir> <target-dir>}"
bins_dir="${2:?usage: coverage-unpack.sh <tests-artifact-dir> <bins-artifact-dir> <target-dir>}"
target_dir="${3:?usage: coverage-unpack.sh <tests-artifact-dir> <bins-artifact-dir> <target-dir>}"

archive="$tests_dir/tests.tar.zst"
test -f "$archive"

# Why: actions/download-artifact has twice delivered a truncated tests.tar.zst
# ("Unexpected EOF in archive" minutes into the untar). The build job records
# the archive's sha256; a mismatch re-downloads the artifact instead of
# failing a shard that ran nothing.
if [ -f "$tests_dir/tests.tar.zst.sha256" ]; then
    for attempt in 1 2 3; do
        (cd "$tests_dir" && sha256sum --check --status tests.tar.zst.sha256) && break
        test "$attempt" -lt 3 || { echo "tests.tar.zst still corrupt after 3 downloads" >&2; exit 1; }
        echo "tests.tar.zst checksum mismatch, re-downloading (attempt $attempt)" >&2
        rm -rf "$tests_dir"/*
        gh run download "$GITHUB_RUN_ID" --name coverage-tests --dir "$tests_dir"
    done
fi
mkdir -p "$target_dir"
tar --zstd -xf "$archive" -C "$target_dir" --strip-components=1 target
rm -f "$archive"

install -m 0755 "$bins_dir/systemprompt" "$bins_dir/systemprompt-bridge" "$target_dir/debug/"
chmod 0755 "$tests_dir/systemprompt-test-migrate"

missing=0
while IFS= read -r binary; do
    if [ ! -x "$binary" ]; then
        echo "missing from the unpacked archive: $binary" >&2
        missing=$((missing + 1))
    fi
done < <(jq -r '."rust-binaries"[]."binary-path"' "$tests_dir/binaries.json" | sort -u)
test "$missing" -eq 0
df -h "$target_dir"
