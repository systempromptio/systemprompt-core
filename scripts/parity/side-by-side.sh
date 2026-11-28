#!/usr/bin/env bash
# The harness beside the unmodified reference agent, over the golden scenarios.
#
#   scripts/parity/side-by-side.sh --out <dir> [--cache <dir>] [--agent <path>]
#       [--reference <dir>] [--only <id or number>]... [--protocol messages|chat]...
#       [--limit <seconds>]
#   scripts/parity/side-by-side.sh --out <dir> --reuse
#
# One command, six steps:
#
#   1. Installs the reference agent at the release pinned in
#      .github/workflows/parity.yml: the same tag, asset and SHA-256, read from
#      that file and verified. The archive is kept under --cache (default
#      ${XDG_CACHE_HOME:-~/.cache}/systemprompt-parity). The pinned asset is
#      the Linux x64 one; on any other system pass --agent <path> to an
#      executable of the pinned version, which skips the download.
#   2. Builds systemprompt-scripted-instance and parity-ledger from crates/tests.
#   3. Runs crates/tests/parity/reference/run.py over the corpus for both
#      protocols into <out>/reference. --reference <dir> takes an export that
#      already exists (the workflow's) and skips steps 1 and 3.
#   4. Runs the golden runner, which drives the harness over the same scenarios
#      and protocols, and copies what it recorded into <out>/harness.
#   5. Runs parity-ledger with the reviewed classifications.toml:
#      <out>/ledger.json, <out>/ledger.md and both normalised traces of every
#      compared run under <out>/traces.
#   6. Writes <out>/side-by-side.html and <out>/summary.tsv (render.py).
#
# --reuse takes <out>/reference and <out>/harness from an earlier run and does
# steps 2, 5 and 6 only: the comparison again, after a change to the
# comparator or to classifications.toml, without running either product.
#
# No model credential is needed, and the command refuses to start when the
# environment holds one (the workflow's check). The workflow's network
# namespace and system-call trace are not applied here.
#
# Exit: 0 when every difference is classified and none is a defect; 1 when a
# difference is unclassified, a defect is open or a run could not be compared;
# 2 for a usage or environment problem.
set -uo pipefail

usage() { sed -n '2,/^set -uo/p' "$0" | sed -e '$d' -e 's/^# \{0,1\}//'; }
die() { echo "side-by-side: $*" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd) || exit 2
workflow="$root/.github/workflows/parity.yml"
manifest="$root/crates/tests/Cargo.toml"
scenarios="$root/crates/tests/golden/scenarios"
runner="$root/crates/tests/parity/reference/run.py"

out="" agent="" reference="" limit="" reuse=""
cache="${XDG_CACHE_HOME:-$HOME/.cache}/systemprompt-parity"
only=() protocols=()
while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --reuse) reuse=1; shift; continue ;;
        --out|--cache|--agent|--reference|--only|--protocol|--limit)
            [ $# -ge 2 ] || die "$1 needs a value"
            case "$1" in
                --out) out=$2 ;;
                --cache) cache=$2 ;;
                --agent) agent=$2 ;;
                --reference) reference=$2 ;;
                --only) only+=("$2") ;;
                --protocol) protocols+=("$2") ;;
                --limit) limit=$2 ;;
            esac
            shift 2 ;;
        *) die "unknown argument $1 (--help lists them)" ;;
    esac
done
[ -n "$out" ] || die "--out is required"
if [ -n "$reuse" ]; then
    [ -z "$reference$agent" ] || die "--reuse takes the reference export from --out; it takes no --reference or --agent"
    [ -f "$out/reference/index.json" ] || die "--reuse needs $out/reference/index.json from an earlier run"
    [ -d "$out/harness" ] || die "--reuse needs $out/harness from an earlier run"
    reference="$out/reference"
fi
for protocol in ${protocols[@]+"${protocols[@]}"}; do
    case "$protocol" in messages|chat) ;; *) die "--protocol is messages or chat, not $protocol" ;; esac
done

pin() { sed -n "s/^  $1: *//p" "$workflow" | head -n 1; }
repository=$(pin REFERENCE_AGENT_REPOSITORY)
tag=$(pin REFERENCE_AGENT_TAG)
asset=$(pin REFERENCE_AGENT_ASSET)
sha256=$(pin REFERENCE_AGENT_SHA256)
binary=$(pin REFERENCE_AGENT_BINARY)
for value in "$repository" "$tag" "$asset" "$sha256" "$binary"; do
    [ -n "$value" ] || die "$workflow does not hold the reference agent's pin"
done

# The workflow's check: a provider name together with a key, token, secret or
# credential suffix, or a cloud identity variable. The agent's own name is
# taken from the pin, as every name of that product is.
own=$(printf '%s' "$binary" | tr '[:lower:]-' '[:upper:]_')
found=$(env | cut -d= -f1 \
    | grep -iE "(^|_)(ANTHROPIC|OPENAI|GEMINI|GOOGLE|VERTEX|AZURE|AWS|BEDROCK|OPENROUTER|MISTRAL|GROQ|XAI|DEEPSEEK|COHERE|OLLAMA|$own)" \
    | grep -iE '(KEY|TOKEN|SECRET|CREDENTIAL|PASSWORD|_ID)$|^GOOGLE_APPLICATION_CREDENTIALS$|^AWS_PROFILE$|^AWS_ROLE_ARN$' || true)
if [ -n "$found" ]; then
    echo "side-by-side: provider variables are set; the comparison runs with none:" >&2
    echo "$found" >&2
    exit 2
fi

# Without ripgrep on PATH the reference agent fetches one during the run.
needs=(python3 cargo git lsof)
[ -n "$reference" ] || needs+=(rg)
[ -n "$reference" ] || [ -n "$agent" ] || needs+=(curl tar)
for tool in "${needs[@]}"; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool is not on PATH"
done

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

install_agent() {
    local system
    system="$(uname -s)-$(uname -m)"
    [ "$system" = "Linux-x86_64" ] \
        || die "the pinned asset ($asset) is the Linux x64 one and this is $system; pass --agent <path> to an executable of $tag"
    local home="$cache/$tag"
    mkdir -p "$home" || die "cannot create $home"
    local archive="$home/$asset"
    if [ ! -f "$archive" ] || [ "$(sha256_of "$archive")" != "$sha256" ]; then
        echo "side-by-side: downloading $asset at $tag"
        curl --fail --silent --show-error --location --retry 5 \
            --output "$archive.part" \
            "https://github.com/$repository/releases/download/$tag/$asset" \
            || die "the download failed"
        mv "$archive.part" "$archive"
    fi
    local got
    got=$(sha256_of "$archive")
    [ "$got" = "$sha256" ] || die "$archive has SHA-256 $got, not the pinned $sha256"
    rm -rf "$home/agent"
    mkdir -p "$home/agent"
    tar -xzf "$archive" -C "$home/agent" || die "cannot unpack $archive"
    [ -x "$home/agent/$binary" ] || die "$asset holds no executable named $binary"
    agent="$home/agent/$binary"
}

mkdir -p "$out" || die "cannot create $out"
out=$(cd "$out" && pwd)
mkdir -p "$cache" || die "cannot create $cache"
cache=$(cd "$cache" && pwd)
[ -n "$reuse" ] || rm -rf "$out/harness"
rm -rf "$out/traces" "$out/work" \
    "$out/ledger.json" "$out/ledger.md" "$out/side-by-side.html" "$out/summary.tsv"

if [ -n "$reference" ]; then
    [ -f "$reference/index.json" ] || die "$reference holds no index.json; it is not a reference export"
    reference=$(cd "$reference" && pwd)
else
    if [ -n "$agent" ]; then
        [ -x "$agent" ] || die "$agent is not an executable"
        agent="$(cd "$(dirname "$agent")" && pwd)/$(basename "$agent")"
    else
        install_agent
    fi
    reference="$out/reference"
    rm -rf "$reference"
fi

cd "$root" || exit 2
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"

echo "side-by-side: building the scripted instance and the comparator"
cargo build --locked --manifest-path "$manifest" \
    -p systemprompt-scripted-instance --bin systemprompt-scripted-instance \
    -p systemprompt-parity-compare --bin parity-ledger \
    || die "the build failed"
target=$(cargo metadata --locked --manifest-path "$manifest" --no-deps --format-version 1 \
    | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])') \
    || die "cargo metadata failed"
instance="$target/debug/systemprompt-scripted-instance"
ledger="$target/debug/parity-ledger"
[ -x "$instance" ] && [ -x "$ledger" ] || die "the build left no executables under $target/debug"

if [ ! -f "$reference/index.json" ]; then
    echo "side-by-side: running the scenarios through the reference agent"
    arguments=(--agent "$agent" --instance "$instance" --scenarios "$scenarios"
        --out "$reference" --work "$out/work")
    for value in ${only[@]+"${only[@]}"}; do arguments+=(--only "$value"); done
    for value in ${protocols[@]+"${protocols[@]}"}; do arguments+=(--protocol "$value"); done
    [ -z "$limit" ] || arguments+=(--limit "$limit")
    python3 "$runner" "${arguments[@]}" \
        || echo "side-by-side: the reference runner reported a run it could not export; it is entered as an error"
    [ -f "$reference/index.json" ] || die "the reference runner wrote no index.json"
    rm -rf "$out/work"
fi
python3 - "$reference/index.json" "$tag" <<'PY' || exit 2
import json, sys
index = json.load(open(sys.argv[1]))
version = index["agent"]["version"]
print("side-by-side: reference agent %s, sha256 %s" % (version, index["agent"]["sha256"]))
if sys.argv[2] not in version.split():
    sys.exit("side-by-side: the agent reports %r, not the pinned %s" % (version, sys.argv[2]))
PY

# The golden runner names its tests s<number>_<id>, so a scenario is selected
# by that prefix; it writes each run under <target>/tmp/golden/runs.
filters=()
for value in ${only[@]+"${only[@]}"}; do
    number=$(printf '%s' "$value" | sed -n 's/^\([0-9][0-9]\).*/\1/p')
    [ -n "$number" ] || die "--only $value does not start with a two-digit scenario number"
    filters+=("s${number}_")
done
if [ -n "$reuse" ]; then
    echo "side-by-side: reusing $out/harness and $reference"
else
runs="$target/tmp/golden/runs"
rm -rf "$runs"
echo "side-by-side: running the scenarios through the harness"
cargo test --locked --manifest-path "$manifest" \
    -p systemprompt-golden-runner --test golden -- ${filters[@]+"${filters[@]}"} \
    || echo "side-by-side: the golden runner reported differences from the expected traces; the ledger lists them"
[ -d "$runs" ] || die "the golden runner wrote nothing under $runs"
cp -R "$runs" "$out/harness"
if [ ${#protocols[@]} -gt 0 ]; then
    for run in "$out"/harness/*/*; do
        [ -d "$run" ] || continue
        case " ${protocols[*]} " in *" $(basename "$run") "*) ;; *) rm -rf "$run" ;; esac
    done
fi
fi

echo "side-by-side: comparing"
# Its standard output is ledger.md, which it also writes; its exit is the
# gate render.py applies below, with open defects added.
"$ledger" --runs "$out/harness" --out "$out" --reference "$reference" --scenarios "$scenarios" \
    > /dev/null || true
[ -f "$out/ledger.json" ] || die "the comparator wrote no ledger"

python3 "$root/scripts/parity/render.py" --out "$out" --reference "$reference" --scenarios "$scenarios"
