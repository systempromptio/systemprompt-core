# Sourced by the rust-contracts gate wrappers (discarded, fail-open,
# swallowed-errors, tracing-messages): runs one scan mode over the core
# production roots, narrowed to the `--paths` scope, or over explicit ROOTs.
# rust-contracts skips any directory literally named `tests` or `target`.

. "$(dirname "${BASH_SOURCE[0]}")/gate-scope.sh"

rust_contracts_gate() {
    local mode="$1" name="$2"
    shift 2
    gate_scope_parse "$@"
    set -- "${GATE_ARGS[@]}"
    if [ "$#" -eq 0 ]; then
        local -a roots
        mapfile -t roots < <(gate_roots "$GATE_ROOT/crates/shared" "$GATE_ROOT/crates/infra" \
            "$GATE_ROOT/crates/domain" "$GATE_ROOT/crates/app" "$GATE_ROOT/crates/entry" \
            "$GATE_ROOT/systemprompt/src" "$GATE_ROOT/bin/bridge/src")
        gate_pass_if_empty "$name" "${#roots[@]}"
        set -- "${roots[@]}"
    fi
    cargo run --quiet --locked --manifest-path "$GATE_ROOT/scripts/rust-contracts/Cargo.toml" -- \
        --cache-dir "$GATE_ROOT/scripts/rust-contracts/target/contracts-cache" "$mode" "$@"
}
