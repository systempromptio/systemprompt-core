#!/usr/bin/env bash
# Prepares the disposable test database: builds the migrate tool first (a build
# failure leaves the database intact), drops and recreates the target database,
# then applies every extension schema.
#
#   scripts/test-db.sh               build the tool OFFLINE (committed .sqlx
#                                    caches), then reset and migrate
#   scripts/test-db.sh --online      build the tool ONLINE against the target
#                                    database as it stands before the reset, so
#                                    a query whose cache entry is not written
#                                    yet still compiles; the database must exist
#   scripts/test-db.sh --print-url   print the URL it targets, change nothing
#   scripts/test-db.sh --editor [NAME]
#                                    give this editor session its own database,
#                                    `systemprompt_test_<NAME>` (NAME defaults to
#                                    the worktree directory), reset and migrate
#                                    it OFFLINE, then print the exports a session
#                                    evaluates: eval "$(scripts/test-db.sh --editor)"
#
# The target is TEST_DATABASE_URL, default a `systemprompt_test` database on
# the local server. The default URL carries no password: psql and sqlx read it
# from PGPASSWORD or ~/.pgpass. Used by `just test-shard`, `just test-rust`, the
# test-workspace phases of scripts/gate.sh and scripts/regen.sh.
#
# `--editor` never targets `systemprompt_test`. It reads the password at run
# time from PGPASSWORD, then ~/.pgpass, then the DATABASE_URL of the
# developer's untracked .cargo/config.toml (this worktree's, then the main
# checkout's), and prints, for the session to export, TEST_DATABASE_URL and
# DATABASE_URL carrying that password (the cli-integration lifecycle tests pass
# it to `admin setup --db-password`) and SYSTEMPROMPT_BIN, the `systemprompt`
# binary in this worktree's cargo target directory that the subprocess tests
# spawn. Nothing is written to a file.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
    echo "usage: $0 [--online|--print-url|--editor [NAME]]" >&2
    exit 2
}

# The password of the first `postgres://user:password@` DATABASE_URL in FILE.
cargo_config_password() {
    [ -f "$1" ] || return 1
    sed -nE 's|^[[:space:]]*DATABASE_URL[[:space:]]*=[[:space:]]*"postgres(ql)?://[^:/@"]+:([^@"]+)@.*|\2|p' "$1" | head -n1 | grep .
}

# The ~/.pgpass password for HOST PORT DB USER (`*` matches any field).
pgpass_password() {
    local file="${PGPASSFILE:-$HOME/.pgpass}" h p d u pw
    [ -f "$file" ] || return 1
    while IFS=: read -r h p d u pw; do
        case "$h" in \#*|"") continue ;; esac
        { [ "$h" = "*" ] || [ "$h" = "$1" ]; } && { [ "$p" = "*" ] || [ "$p" = "$2" ]; } \
            && { [ "$d" = "*" ] || [ "$d" = "$3" ]; } && { [ "$u" = "*" ] || [ "$u" = "$4" ]; } \
            && { printf '%s\n' "$pw"; return 0; }
    done < "$file"
    return 1
}

editor_password() {
    local host="$1" port="$2" db="$3" user="$4" main
    [ -z "${PGPASSWORD:-}" ] || { printf '%s\n' "$PGPASSWORD"; return 0; }
    pgpass_password "$host" "$port" "$db" "$user" && return 0
    cargo_config_password .cargo/config.toml && return 0
    main="$(cd "$(git rev-parse --path-format=absolute --git-common-dir)/.." && pwd)"
    cargo_config_password "$main/.cargo/config.toml" && return 0
    return 1
}

default_db="postgres://systemprompt_admin@localhost:5432/systemprompt_test"
db="${TEST_DATABASE_URL:-$default_db}"
build_env=(SQLX_OFFLINE=true)
editor=0
case "${1:-}" in
    --print-url) [ "$#" -eq 1 ] || usage; printf '%s\n' "$db"; exit 0 ;;
    --online) build_env=(SQLX_OFFLINE=false "DATABASE_URL=${db}"); shift ;;
    --editor)
        editor=1
        raw="${2:-$(basename "$(git rev-parse --show-toplevel)")}"
        [ "$#" -le 2 ] || usage
        set --
        suffix="$(printf '%s' "$raw" | tr '[:upper:]-' '[:lower:]_' | tr -cd 'a-z0-9_')"
        [ -n "$suffix" ] || { echo "test-db: --editor NAME must hold a letter or digit" >&2; exit 2; }
        server="${db%/*}"
        hostport="${server##*@}"
        user="${server#*://}"; user="${user%%@*}"; user="${user%%:*}"
        server="postgres://${user}@${hostport}"
        db="${server}/systemprompt_test_${suffix}"
        name="${db##*/}"
        if ! password="$(editor_password "${hostport%%:*}" "${hostport##*:}" "$name" "$user")"; then
            echo "test-db: no database password: set PGPASSWORD, add a ~/.pgpass line, or give" >&2
            echo "         .cargo/config.toml a DATABASE_URL that carries one" >&2
            exit 1
        fi
        export PGPASSWORD="$password"
        ;;
    "") ;;
    *) usage ;;
esac
[ "$#" -eq 0 ] || usage

base="${db%/*}"
name="${db##*/}"
echo "▶ building migration tool (${build_env[0]})" >&2
migrate_bin=$(env "${build_env[@]}" cargo build --manifest-path crates/tests/Cargo.toml \
    -p systemprompt-test-migrate --message-format=json-render-diagnostics \
    | jq -r 'select(.reason == "compiler-artifact" and .executable != null) | .executable' | tail -n1)
test -x "${migrate_bin}"
echo "▶ resetting test database: ${name}" >&2
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
echo "▶ applying extension schemas" >&2
SQLX_OFFLINE=true DATABASE_URL="${db}" "${migrate_bin}"

if [ "$editor" -eq 1 ]; then
    target_dir="$(cargo metadata --offline --no-deps --format-version 1 | jq -r .target_directory)"
    bin="${target_dir}/debug/systemprompt"
    [ -x "$bin" ] || echo "test-db: ${bin} is not built yet; the lifecycle tests need it: cargo build -p systemprompt-cli --bin systemprompt" >&2
    url="postgres://${user}:${password}@${hostport}/${name}"
    printf 'export TEST_DATABASE_URL=%q\n' "$url"
    printf 'export DATABASE_URL=%q\n' "$url"
    printf 'export SYSTEMPROMPT_BIN=%q\n' "$bin"
    echo "test-db: ${name} is migrated; the exports above point this session at it" >&2
fi
