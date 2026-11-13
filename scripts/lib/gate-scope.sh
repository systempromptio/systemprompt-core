# Sourced by every unit source gate: the path-scope convention, file
# enumeration, allowlist reading and the empty-scope pass. The scope rules
# themselves live once in gate_scope.py beside this file.
#
#   . "$(dirname "${BASH_SOURCE[0]}")/lib/gate-scope.sh"
#   gate_scope_parse "$@"; set -- "${GATE_ARGS[@]}"
#
# A gate's own arguments come first; an optional trailing `--paths P [P ...]`
# narrows it to files at or under those paths. Without `--paths` the gate is in
# full mode (whole repository, stale-allowlist checks included).

GATE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATE_SCOPE_PY="$GATE_ROOT/scripts/lib/gate_scope.py"
GATE_ARGS=()
GATE_PATHS=()
GATE_SCOPED=0

gate_scope_parse() {
    GATE_ARGS=()
    GATE_PATHS=()
    GATE_SCOPED=0
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--paths" ]; then
            shift
            [ "$#" -gt 0 ] || { echo "gate-scope: --paths needs at least one path" >&2; exit 2; }
            GATE_SCOPED=1
            GATE_PATHS=("$@")
            return 0
        fi
        GATE_ARGS+=("$1")
        shift
    done
}

gate_full_mode() {
    [ "$GATE_SCOPED" -eq 0 ]
}

gate_scope_py() {
    local -a flags=()
    gate_full_mode || flags=(--paths "${GATE_PATHS[@]}")
    python3 -B "$GATE_SCOPE_PY" "$@" "${flags[@]}"
}

# Tracked and untracked-not-ignored files matching the git pathspecs, in scope,
# root-relative, sorted, one per line.
gate_files() {
    gate_scope_py files "$@"
}

# The given default roots narrowed to the scope (all of them in full mode).
gate_roots() {
    if gate_full_mode; then
        [ "$#" -eq 0 ] || printf '%s\n' "$@"
    else
        gate_scope_py roots "$@"
    fi
}

# The given directories, unnarrowed, that overlap the scope (all in full mode).
gate_touching() {
    if gate_full_mode; then
        [ "$#" -eq 0 ] || printf '%s\n' "$@"
    else
        gate_scope_py touching "$@"
    fi
}

# Keeps the stdin lines whose leading `path` (up to the first `:`) is in scope.
gate_filter() {
    if gate_full_mode; then
        cat
    else
        gate_scope_py filter
    fi
}

# Allowlist entries: each line with its `#` comment and trailing blanks removed,
# blank results skipped.
gate_allowlist() {
    local line
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line%%#*}"
        line="${line%"${line##*[![:space:]]}"}"
        [ -n "$line" ] && printf '%s\n' "$line"
    done < "$1"
    return 0
}

# Two scope operations, owned here and never mixed:
#
#   --paths P...  narrows THIS repository's default roots (unit-gate scope).
#   ROOT...       positional; scans ANOTHER tree (an install, a test fixture) in
#                 place of the defaults, with its own allowlist.
#
# A gate that accepts ROOTs calls gate_require_one_scope with its ROOT count
# before choosing roots, so `ROOT... --paths P...` is refused rather than one
# silently winning.
gate_require_one_scope() {
    if [ "$1" -gt 0 ] && ! gate_full_mode; then
        echo "gate-scope: explicit ROOTs scan another tree and --paths narrows this one; pass one, not both" >&2
        exit 2
    fi
}

# Every file under each explicit ROOT (a file or directory, inside this
# repository or not), printed as ROOT/<path>: tracked plus untracked-not-ignored
# when ROOT sits in a git work tree, every file otherwise. The alternate-tree
# form of the gates that accept ROOT arguments.
gate_tree_files() {
    local r
    for r in "$@"; do
        if git -C "$r" rev-parse --show-toplevel >/dev/null 2>&1; then
            git -C "$r" ls-files -co --exclude-standard | sed "s|^|${r%/}/|"
        else
            find "$r" -type f
        fi
    done | while IFS= read -r f; do [ -f "$f" ] && printf '%s\n' "$f"; done
}

# In scoped mode, passes the gate when nothing in scope is its concern.
gate_pass_if_empty() {
    local name="$1" count="$2"
    if ! gate_full_mode && [ "$count" -eq 0 ]; then
        echo "$name: OK (nothing in scope: ${GATE_PATHS[*]})"
        exit 0
    fi
}
