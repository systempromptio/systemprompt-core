#!/usr/bin/env bash
# Strict / report mode of lint-raw-ids (invoked by lint-raw-ids.sh).
#
# The forbidden name set is derived, not hand-kept: every identifier type in
# crates/shared/identifiers/src (`define_id!`, `define_token!`, and the
# hand-written `pub struct Name(String)` newtypes whose snake name ends in
# `_id`, `_name` or `_code`) contributes its snake_case name
# (`McpExecutionId` => `mcp_execution_id`), plus the NAME_TABLE below for
# name-like identities and aliases whose field name is not the type name.
# A field or fn argument with one of those names typed `String`, `&str`,
# `&'a str`, `Option<String>`, `Option<&str>`, `Uuid` or `Option<Uuid>` is
# reported.
#
# Usage: lint-raw-ids-strict.sh [--report]
#   (default)  print every offending line, exit 1 if any
#   --report   print counts per crate and per name, always exit 0
set -uo pipefail

REPORT=0
for arg in "$@"; do
    case "$arg" in
        --report) REPORT=1 ;;
        --strict) ;;
        *) echo "lint-raw-ids-strict: unknown argument '$arg'" >&2; exit 2 ;;
    esac
done

if ! command -v rg >/dev/null 2>&1; then
    echo "lint-raw-ids --strict/--report requires ripgrep (rg); install it and rerun" >&2
    exit 2
fi

IDS_DIR=crates/shared/identifiers/src

# field/arg name -> canonical type. Rows whose name is derivable from the type
# are omitted (plugin_id, skill_id, rule_id, ... come from the derived set).
NAME_TABLE=(
    "agent_name:AgentName"
    "service_name:ServiceName"
    "server_name:McpServerId"        # an MCP server's name is its id
    "mcp_server_name:McpServerId"
    "tool_name:McpToolName"
    "host_id:HostKind"               # closed enum, systemprompt_models::bridge::host
    "requested_by:UserId"
    "approver_id:UserId"
    "owner_user_id:UserId"
    "jti:AccessTokenId"              # a JWT's jti is the access token's id
)

snake() {
    sed -E 's/([a-z0-9])([A-Z])/\1_\2/g; s/([A-Z])([A-Z][a-z])/\1_\2/g' | tr '[:upper:]' '[:lower:]'
}

DERIVED=$(
    {
        rg --no-filename -o -r '$1' 'define_(?:id|token)!\(\s*([A-Z][A-Za-z0-9]*)' "$IDS_DIR" -g '!macros/**'
        rg --no-filename -o -r '$1' 'pub struct ([A-Z][A-Za-z0-9]*)\(String\)' "$IDS_DIR" -g '!macros/**' \
            | snake | rg '_(id|name|code)$' | while IFS= read -r n; do printf '%s\n' "@$n"; done
    } | while IFS= read -r t; do
        case "$t" in
            @*) printf '%s\n' "${t#@}" ;;
            *) printf '%s\n' "$t" | snake ;;
        esac
    done
)

TABLE_NAMES=$(for row in "${NAME_TABLE[@]}"; do printf '%s\n' "${row%%:*}"; done)

NAMES=$(printf '%s\n%s\n' "$DERIVED" "$TABLE_NAMES" | rg -v '^$' | sort -u)
NAME_COUNT=$(printf '%s\n' "$NAMES" | wc -l | tr -d ' ')
ALT=$(printf '%s\n' "$NAMES" | paste -sd '|' -)

PATTERN="\\b(${ALT})\\s*:\\s*(?:Option<\\s*)?(?:&\\s*(?:'[a-z_]+\\s+)?(?:mut\\s+)?)?(?:uuid::)?(?:String|str|Uuid)\\s*(?:[,>)=;{]|\$)"

SEARCH_DIRS=(crates/shared crates/infra crates/domain crates/app crates/entry systemprompt/src bin/bridge/src)

RAW=$(rg -n --no-heading --color=never \
    -g '*.rs' \
    -g '!crates/tests/**' \
    -g '!**/target/**' \
    -g '!**/.sqlx/**' \
    -g '!crates/entry/api/src/routes/oauth/**' \
    -g '!crates/domain/mcp/src/middleware/session_handler/session_store.rs' \
    -g '!crates/shared/models/src/wire/**' \
    -e "$PATTERN" \
    "${SEARCH_DIRS[@]}" 2>/dev/null || true)

MATCHES=""
while IFS= read -r line; do
    [ -z "$line" ] && continue
    file="${line%%:*}"
    rest="${line#*:}"
    lineno="${rest%%:*}"
    content="${rest#*:}"
    trimmed="${content#"${content%%[![:space:]]*}"}"
    case "$trimmed" in
        //*) continue ;;
    esac
    MATCHES+="${file}:${lineno}:${content}"$'\n'
done <<< "$RAW"

if [ "$REPORT" -eq 1 ]; then
    total=$(printf '%s' "$MATCHES" | rg -c '' || true)
    echo "lint-raw-ids report: ${NAME_COUNT} forbidden names (derived from ${IDS_DIR} + name table), ${total:-0} offending lines"
    echo ""
    echo "By crate (lines):"
    printf '%s' "$MATCHES" | cut -d: -f1 | awk -F/ '
        $1 == "crates" { print $1 "/" $2 "/" $3; next }
        $1 == "bin"    { print $1 "/" $2; next }
        { print $1 }' | sort | uniq -c | sort -rn
    echo ""
    echo "By name (matches):"
    printf '%s' "$MATCHES" | cut -d: -f3- | rg -o -r '$1' "$PATTERN" | sort | uniq -c | sort -rn
    exit 0
fi

if [ -z "$MATCHES" ]; then
    echo "lint-raw-ids --strict: OK (no raw ID fields or arguments found; ${NAME_COUNT} names checked)"
    exit 0
fi

echo "lint-raw-ids --strict: raw String/&str/Uuid used for typed-ID field or argument names:"
echo ""
printf '%s' "$MATCHES"
exit 1
