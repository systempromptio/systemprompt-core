#!/usr/bin/env bash
# Build report: per reference composition (a facade feature set of the
# `systemprompt` crate, listed in scripts/build-compositions.txt) it
# reports the crate closure and, unless --closure-only, the stripped release
# binary size. The composition marked `largest` also gets `cargo bloat --crates`
# and `cargo build --timings`. A measurement, never a gate.
#
# Each composition is generated under target/build-report/<name>/ as a crate
# with its own empty `[workspace]`, its own target dir, the root
# `[profile.release]` copied verbatim, a path dependency on the facade, and a
# copy of the root Cargo.lock. Cargo runs without `--locked`: the only change
# it makes to the copied lock is adding the composition package and pruning
# entries outside its closure; the script fails if the lock then names any
# package version the root lock does not, so every composition builds
# the exact dependency versions the workspace pins.
#
# Usage: scripts/build-report.sh [--closure-only] [--offline] [--only NAME] [OUTPUT.md]
#   OUTPUT.md defaults to target/build-report/report.md; the JSON sidecar is
#   written next to it as <OUTPUT>.json, the timings HTML as
#   target/build-report/cargo-timing.html.
# Closure-only mode runs only `cargo metadata` and compiles nothing; --offline
# passes through to every cargo call. Both closure counts come from one
# host-filtered resolve (scripts/lib/cargo_graph.py closure), so an --offline
# and an online run over the same lock report the same closure.
# SQLX_OFFLINE defaults to true (per-crate .sqlx caches).
# Each composition's target dir is removed once measured, so a runner's disk
# holds one composition at a time; BUILD_REPORT_KEEP_TARGET=1 keeps them.
set -euo pipefail
cd "$(dirname "$0")/.."
root="$PWD"

closure_only=false
offline=()
only=""
out=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --closure-only) closure_only=true ;;
        --offline) offline=(--offline) ;;
        --only) only="${2:?--only needs a composition name}"; shift ;;
        -h|--help) sed -n '2,23p' "$0"; exit 0 ;;
        -*) echo "unknown flag: $1" >&2; exit 2 ;;
        *) out="$1" ;;
    esac
    shift
done

export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"
command -v jq >/dev/null || { echo 'jq is not installed' >&2; exit 2; }

table="$root/scripts/build-compositions.txt"
base="$root/target/build-report"
out="${out:-$base/report.md}"
json="${out%.md}.json"
mkdir -p "$base" "$(dirname "$out")"

mapfile -t rows < <(grep -vE '^\s*(#|$)' "$table")
[ "$(printf '%s\n' "${rows[@]}" | awk '$3 == "largest"' | wc -l)" -eq 1 ] \
    || { echo "$table must mark exactly one composition largest" >&2; exit 2; }
if [ -n "$only" ] && ! printf '%s\n' "${rows[@]}" | awk -v n="$only" '$1 == n {f=1} END {exit !f}'; then
    echo "no composition named $only in $table" >&2; exit 2
fi


release_profile="$(awk '/^\[profile\.release\]/ {p=1; print; next} /^\[/ {p=0} p' "$root/Cargo.toml")"
[ -n "$release_profile" ] || { echo 'no [profile.release] in Cargo.toml' >&2; exit 2; }
tokio_req="$(sed -n 's/^tokio = //p' "$root/Cargo.toml")"
anyhow_req="$(sed -n 's/^anyhow = //p' "$root/Cargo.toml")"

lock_packages() {
    awk '/^\[\[package\]\]/ {if (n) print n" "v; n=""; v=""} /^name = / {n=$3} /^version = / {v=$3} END {if (n) print n" "v}' "$1" \
        | grep -v "^\"$2\" " | sort
}

generate() {
    local features="$1" cli="$2" dir="$3" pkg="$4"
    mkdir -p "$dir/src"
    local feature_list
    feature_list="$(printf '%s' "$features" | jq -Rc 'split(",")')"
    {
        printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n' "$pkg"
        printf '[workspace]\n\n[dependencies]\n'
        printf 'systemprompt = { path = "../../../systemprompt", default-features = false, features = %s }\n' "$feature_list"
        if [ "$cli" = true ]; then
            printf 'tokio = %s\nanyhow = %s\n' "$tokio_req" "$anyhow_req"
        fi
        printf '\n%s\n' "$release_profile"
    } >"$dir/Cargo.toml"
    if [ "$cli" = true ]; then
        cat >"$dir/src/main.rs" <<'EOF'
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Box::pin(systemprompt::cli::run()).await
}
EOF
    else
        cat >"$dir/src/main.rs" <<'EOF'
use systemprompt as _;

fn main() {}
EOF
    fi
    cp "$root/Cargo.lock" "$dir/Cargo.lock"
}

check_lock() {
    local dir="$1" pkg="$2"
    comm -13 <(lock_packages "$root/Cargo.lock" "$pkg") <(lock_packages "$dir/Cargo.lock" "$pkg") >"$dir/lock.diff"
    if [ -s "$dir/lock.diff" ]; then
        echo "composition lock for $pkg resolves versions the root Cargo.lock does not pin; see $dir/lock.diff" >&2
        exit 1
    fi
}

host="$(rustc -vV | sed -n 's/^host: //p')"
[ -n "$host" ] || { echo 'cannot read the host target from rustc -vV' >&2; exit 2; }

results="[]"
bloat_md=""
for row in "${rows[@]}"; do
    read -r name features flag <<<"$row"
    [ -n "$only" ] && [ "$name" != "$only" ] && continue
    pkg="build-report-$name"
    dir="$base/$name"
    cli=false
    case ",$features," in *,cli,*|*,full,*|*,runtime,*) cli=true ;; esac
    generate "$features" "$cli" "$dir" "$pkg"
    echo "== $name ($features)" >&2

    meta="$(cd "$dir" && cargo metadata "${offline[@]}" --format-version 1)"
    check_lock "$dir" "$pkg"
    all_platforms="$(jq '[.resolve.nodes[]] | length - 1' <<<"$meta")"
    host_meta="$(cd "$dir" && cargo metadata "${offline[@]}" --format-version 1 --filter-platform "$host")"
    check_lock "$dir" "$pkg"
    read -r packages members < <(python3 -B "$root/scripts/lib/cargo_graph.py" closure <<<"$host_meta")

    size=null
    if ! $closure_only; then
        build_env=()
        [ "$flag" = largest ] && build_env=(CARGO_PROFILE_RELEASE_STRIP=false)
        timings=()
        [ "$flag" = largest ] && timings=(--timings)
        (cd "$dir" && env "${build_env[@]}" cargo build "${offline[@]}" --release "${timings[@]}")
        check_lock "$dir" "$pkg"
        strip --strip-all -o "$dir/target/release/$pkg.stripped" "$dir/target/release/$pkg"
        size="$(stat -c %s "$dir/target/release/$pkg.stripped")"
        if [ "$flag" = largest ]; then
            cp "$dir/target/cargo-timings/cargo-timing.html" "$base/cargo-timing.html"
            (cd "$dir" && env "${build_env[@]}" cargo bloat --release --crates -n 40 --message-format json) >"$base/bloat.json"
            bloat_md="$(jq -r '"| Crate | Size (bytes) |\n|---|---:|", (.crates[] | "| \(.name) | \(.size) |")' "$base/bloat.json")"
        fi
        [ "${BUILD_REPORT_KEEP_TARGET:-}" = 1 ] || rm -rf "$dir/target"
    fi

    results="$(jq -c --arg n "$name" --arg f "$features" --arg l "${flag:-}" \
        --argjson p "$packages" --argjson m "$members" --argjson a "$all_platforms" --argjson s "$size" \
        '. + [{name: $n, features: ($f | split(",")), largest: ($l == "largest"),
               packages: $p, workspace_packages: $m, all_platform_packages: $a, stripped_bytes: $s}]' <<<"$results")"
done

rev="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
jq -n --arg rev "$rev" --argjson closure_only "$closure_only" --argjson c "$results" \
    '{revision: $rev, closure_only: $closure_only, compositions: $c}' >"$json"

{
    echo "# Build report ($rev)"
    echo
    echo "Packages: resolved normal+build closure for the host target, excluding the composition crate. Workspace: path packages from this repository. All platforms: \`cargo metadata\` resolve nodes for every target."
    echo
    echo "| Composition | Features | Packages | Workspace | All platforms | Stripped binary |"
    echo "|---|---|---:|---:|---:|---:|"
    jq -r '.compositions[] | "| \(.name)\(if .largest then " (largest)" else "" end) | `\(.features | join(","))` | \(.packages) | \(.workspace_packages) | \(.all_platform_packages) | \(if .stripped_bytes == null then "n/a" else "\(.stripped_bytes / 1048576 * 10 | round / 10) MiB" end) |"' "$json"
    if [ -n "$bloat_md" ]; then
        echo
        echo "## cargo bloat --crates (largest composition)"
        echo
        echo "$bloat_md"
    fi
} >"$out"

cat "$out"
