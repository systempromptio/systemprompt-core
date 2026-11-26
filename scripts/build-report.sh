#!/usr/bin/env bash
# Build report: per supported composition (a facade feature set of the
# `systemprompt` crate, listed in scripts/build-compositions.txt) it
# reports the crate closure and, unless --closure-only, the stripped release
# binary size. The composition marked `largest` also gets `cargo bloat --crates`
# and `cargo build --timings`. A measurement, never a gate.
#
# --dev-loop also records, per composition, the developer loop in the root
# workspace: wall seconds of `cargo check` and `cargo clippy` on the facade
# with the composition's features, each from an empty target dir and again
# after touching one leaf source file (systemprompt/src/lib.rs), with
# incremental compilation on and no compiler wrapper.
#
# --units [OUTPUT.md] measures the test loop instead of the compositions: for
# each unit of scripts/units.txt, the test-workspace packages that reach the
# unit's packages (scripts/affected.py --crates), the wall seconds of
# `cargo nextest archive` of them from an empty target dir and of
# `cargo nextest run` of that archive. It needs DATABASE_URL naming a
# migrated database (the test workspace has no offline sqlx cache) and writes
# target/build-report/units.md and units.json by default.
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
# The "touched" half of the developer loop is measured only on the lines
# flagged `largest` and `smallest`; every other line reports clean times.
#
# A CI run splits the work: --list prints the composition names (or, with
# --units, the unit names) as a JSON array for a job matrix, each matrix job
# measures one with --only, and --merge DIR renders the report from the JSON
# sidecars the jobs wrote into DIR (and DIR/bloat.json when present).
#
# Usage: scripts/build-report.sh [--closure-only] [--dev-loop] [--offline] [--only NAME] [OUTPUT.md]
#        scripts/build-report.sh --units [--only UNIT] [OUTPUT.md]
#        scripts/build-report.sh [--units] --list
#        scripts/build-report.sh [--units] [--dev-loop] --merge DIR [OUTPUT.md]
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
dev_loop=false
units_mode=false
offline=()
only=""
out=""
list_only=false
merge_dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --closure-only) closure_only=true ;;
        --dev-loop) dev_loop=true ;;
        --units) units_mode=true ;;
        --offline) offline=(--offline) ;;
        --only) only="${2:?--only needs a composition name}"; shift ;;
        --list) list_only=true ;;
        --merge) merge_dir="${2:?--merge needs a directory}"; shift ;;
        -h|--help) sed -n '2,52p' "$0"; exit 0 ;;
        -*) echo "unknown flag: $1" >&2; exit 2 ;;
        *) out="$1" ;;
    esac
    shift
done

export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"
command -v jq >/dev/null || { echo 'jq is not installed' >&2; exit 2; }

table="$root/scripts/build-compositions.txt"
base="$root/target/build-report"

now() { date +%s.%N; }
elapsed() { awk -v a="$1" -v b="$(now)" 'BEGIN {printf "%.1f", b - a}'; }

timed() {
    local start
    start="$(now)"
    "$@" >&2
    elapsed "$start"
}

render_units() {
    local rev
    rev="$(jq -r .revision "$json")"
    {
        echo "# Test loop report ($rev)"
        echo
        echo "Per unit of scripts/units.txt: the test-workspace packages reaching it, \`cargo nextest archive\` seconds from an empty target dir, and \`cargo nextest run\` seconds of that archive (exit status beside it)."
        echo
        echo "| Unit | Test packages | Archive (s) | Run (s) | Run exit |"
        echo "|---|---:|---:|---:|---:|"
        jq -r '.units[] | "| \(.unit) | \(.test_packages) | \(.archive_seconds // "n/a") | \(.run_seconds // "n/a") | \(.run_exit // "n/a") |"' "$json"
    } >"$out"
    cat "$out"
}

if $units_mode; then
    out="${out:-$base/units.md}"
    json="${out%.md}.json"
    mapfile -t unit_names < <(grep -vE '^\s*(#|$)' "$root/scripts/units.txt" | awk '{print $2}' | awk '!seen[$0]++')
    if $list_only; then
        jq -nc '$ARGS.positional' --args "${unit_names[@]}"
        exit 0
    fi
    mkdir -p "$base/units" "$(dirname "$out")"
    if [ -n "$merge_dir" ]; then
        jq -s '{revision: (.[0].revision // "unknown"), units: (map(.units) | add // [])}' "$merge_dir"/*.json >"$json"
        render_units
        exit 0
    fi
    command -v cargo-nextest >/dev/null || { echo 'cargo-nextest is not installed' >&2; exit 2; }
    [ -n "${DATABASE_URL:-}" ] || { echo '--units needs DATABASE_URL naming a migrated database' >&2; exit 2; }
    export SQLX_OFFLINE=false
    results="[]"
    for unit in "${unit_names[@]}"; do
        [ -n "$only" ] && [ "$unit" != "$only" ] && continue
        mapfile -t unit_pkgs < <(grep -vE '^\s*(#|$)' "$root/scripts/units.txt" | awk -v u="$unit" '$2 == u {print $1}')
        mapfile -t test_pkgs < <(python3 -B "$root/scripts/affected.py" --crates "${unit_pkgs[@]}" --format json | jq -r '.test_packages[]')
        echo "== unit $unit (${#test_pkgs[@]} test packages)" >&2
        slug="${unit//:/-}"
        archive_s=null run_s=null run_exit=null
        if [ "${#test_pkgs[@]}" -gt 0 ]; then
            unit_target="$base/units/$slug-target"
            archive="$base/units/$slug.tar.zst"
            rm -rf "$unit_target"
            pkg_args=()
            for p in "${test_pkgs[@]}"; do pkg_args+=(-p "$p"); done
            archive_s="$(timed env CARGO_TARGET_DIR="$unit_target" cargo nextest archive "${offline[@]}" --locked \
                --manifest-path crates/tests/Cargo.toml --lib --tests "${pkg_args[@]}" --archive-file "$archive")"
            start="$(now)"
            run_exit=0
            cargo nextest run --archive-file "$archive" --workspace-remap "$root/crates/tests" \
                --config-file "$root/crates/tests/.config/nextest.toml" \
                --extract-to "$base/units/$slug-extract" --extract-overwrite --no-fail-fast >&2 || run_exit=$?
            run_s="$(elapsed "$start")"
            [ "${BUILD_REPORT_KEEP_TARGET:-}" = 1 ] || rm -rf "$unit_target" "$archive" "$base/units/$slug-extract"
        fi
        results="$(jq -c --arg u "$unit" --argjson n "${#test_pkgs[@]}" --argjson a "$archive_s" \
            --argjson r "$run_s" --argjson e "$run_exit" \
            '. + [{unit: $u, test_packages: $n, archive_seconds: $a, run_seconds: $r, run_exit: $e}]' <<<"$results")"
    done
    rev="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    jq -n --arg rev "$rev" --argjson u "$results" '{revision: $rev, units: $u}' >"$json"
    render_units
    exit 0
fi

dev_loop_measure() {
    local features="$1" bin="$2" touched="$3" target="$base/dev-loop-target"
    local check_clean check_touched=null clippy_clean clippy_touched=null
    local cargo=(env -u RUSTC_WRAPPER CARGO_INCREMENTAL=1 CARGO_TARGET_DIR="$target" cargo)
    local args=(-p systemprompt --no-default-features --features "$features" "${offline[@]}" --locked)
    if [ "$bin" = true ]; then args+=(--bin systemprompt); else args+=(--lib); fi
    rm -rf "$target"
    check_clean="$(timed "${cargo[@]}" check "${args[@]}")"
    if [ "$touched" = true ]; then
        touch "$root/systemprompt/src/lib.rs"
        check_touched="$(timed "${cargo[@]}" check "${args[@]}")"
    fi
    rm -rf "$target"
    clippy_clean="$(timed "${cargo[@]}" clippy "${args[@]}" -- -D warnings)"
    if [ "$touched" = true ]; then
        touch "$root/systemprompt/src/lib.rs"
        clippy_touched="$(timed "${cargo[@]}" clippy "${args[@]}" -- -D warnings)"
    fi
    rm -rf "$target"
    jq -nc --argjson a "$check_clean" --argjson b "$check_touched" --argjson c "$clippy_clean" --argjson d "$clippy_touched" \
        '{check_clean_seconds: $a, check_touched_seconds: $b, clippy_clean_seconds: $c, clippy_touched_seconds: $d}'
}

out="${out:-$base/report.md}"
json="${out%.md}.json"
mkdir -p "$base" "$(dirname "$out")"

mapfile -t rows < <(grep -vE '^\s*(#|$)' "$table")
if $list_only; then
    printf '%s\n' "${rows[@]}" | awk '{print $1}' | jq -Rsc 'split("\n") | map(select(length > 0))'
    exit 0
fi
[ "$(printf '%s\n' "${rows[@]}" | awk '$3 ~ /(^|,)largest(,|$)/' | wc -l)" -eq 1 ] \
    || { echo "$table must mark exactly one composition largest" >&2; exit 2; }
if [ -n "$only" ] && ! printf '%s\n' "${rows[@]}" | awk -v n="$only" '$1 == n {f=1} END {exit !f}'; then
    echo "no composition named $only in $table" >&2; exit 2
fi


release_profile="$(awk '/^\[profile\.release\]/ {p=1; print; next} /^\[/ {p=0} p' "$root/Cargo.toml")"
[ -n "$release_profile" ] || { echo 'no [profile.release] in Cargo.toml' >&2; exit 2; }

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
        printf '\n%s\n' "$release_profile"
    } >"$dir/Cargo.toml"
    if [ "$cli" = true ]; then
        cat >"$dir/src/main.rs" <<'EOF'
fn main() -> Result<std::process::ExitCode, systemprompt::RunError> {
    systemprompt::reference().run()
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
if [ -n "$merge_dir" ]; then
    rows=()
    results="$(jq -s 'map(.compositions) | add // []' $(find "$merge_dir" -name '*.json' ! -name 'bloat.json' | sort))"
    if [ -f "$merge_dir/bloat.json" ]; then
        bloat_md="$(jq -r '"| Crate | Size (bytes) |\n|---|---:|", (.crates[] | "| \(.name) | \(.size) |")' "$merge_dir/bloat.json")"
    fi
fi
for row in "${rows[@]}"; do
    read -r name features flags <<<"$row"
    flag=""
    case ",${flags:-}," in *,largest,*) flag=largest ;; esac
    [ -n "$only" ] && [ "$name" != "$only" ] && continue
    pkg="build-report-$name"
    dir="$base/$name"
    cli=false
    case ",${flags:-}," in *,boot,*) cli=true ;; esac
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

    loop=null
    if $dev_loop; then
        touched=false
        case ",${flags:-}," in *,largest,*|*,smallest,*) touched=true ;; esac
        loop="$(dev_loop_measure "$features" "$cli" "$touched")"
    fi

    results="$(jq -c --arg n "$name" --arg f "$features" --arg l "${flag:-}" \
        --argjson p "$packages" --argjson m "$members" --argjson a "$all_platforms" --argjson s "$size" \
        --argjson d "$loop" \
        '. + [{name: $n, features: ($f | split(",")), largest: ($l == "largest"),
               packages: $p, workspace_packages: $m, all_platform_packages: $a, stripped_bytes: $s,
               dev_loop: $d}]' <<<"$results")"
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
    if $dev_loop; then
        echo
        echo "## Developer loop"
        echo
        echo "Wall seconds in the root workspace, \`-p systemprompt\` with the composition's features (\`--bin systemprompt\` for a boot line, \`--lib\` otherwise): from an empty target dir, and (on the largest and smallest lines) after touching \`systemprompt/src/lib.rs\`. Incremental on, no compiler wrapper; clippy runs with \`-D warnings\`."
        echo
        echo "| Composition | check clean | check touched | clippy clean | clippy touched |"
        echo "|---|---:|---:|---:|---:|"
        jq -r '.compositions[] | select(.dev_loop != null) | "| \(.name) | \(.dev_loop.check_clean_seconds) | \(.dev_loop.check_touched_seconds // "n/a") | \(.dev_loop.clippy_clean_seconds) | \(.dev_loop.clippy_touched_seconds // "n/a") |"' "$json"
    fi
    if [ -n "$bloat_md" ]; then
        echo
        echo "## cargo bloat --crates (largest composition)"
        echo
        echo "$bloat_md"
    fi
} >"$out"

cat "$out"
