#!/usr/bin/env bash
# Gate: no raw String/&str/Uuid for an identity that has a typed identifier.
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
# Usage: lint-raw-ids.sh [--report]
#   (default)  print every offending line, exit 1 if any
#   --report   print counts per crate and per name, always exit 0
set -uo pipefail

REPORT=0
for arg in "$@"; do
    case "$arg" in
        --report) REPORT=1 ;;
        *) echo "lint-raw-ids: unknown argument '$arg'" >&2; exit 2 ;;
    esac
done

if ! command -v rg >/dev/null 2>&1; then
    echo "lint-raw-ids requires ripgrep (rg); install it and rerun" >&2
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
    "ext_id:ExtensionId"
)

# Boundary values: `path|names|reason`. A listed name in the listed file is a
# value owned by someone else's format (persisted JSON that historical rows
# must keep decoding, an external wire field, an LLM-authored plan) or is not
# the identity its name suggests. Every entry must still match a line; a
# stale entry fails the gate so the list only shrinks.
BOUNDARY=(
    "crates/shared/models/src/a2a/task_metadata.rs|agent_name tool_name mcp_server_name|persisted A2A TaskMetadata JSON; historical rows hold unknown/unset names"
    "crates/shared/models/src/a2a/artifact_metadata.rs|tool_name skill_name|persisted A2A artifact metadata JSON"
    "crates/shared/models/src/a2a/artifact_summary.rs|tool_name|listing of the persisted artifact metadata tool_name column"
    "crates/domain/agent/src/models/database_rows.rs|tool_name skill_name|task_artifacts decode row feeding the persisted A2A artifact metadata"
    "crates/shared/models/src/execution/step/content.rs|tool_name skill_name|persisted execution-step content JSON"
    "crates/shared/models/src/artifacts/tool_result/mod.rs|tool_name server_name|persisted tool_result artifact JSON"
    "crates/shared/models/src/events/a2a_event.rs|agent_name|persisted A2A event payload"
    "crates/shared/models/src/events/payloads/a2a.rs|agent_name|persisted A2A event payload"
    "crates/shared/models/src/agui/payloads.rs|skill_name|AG-UI event payload (persisted event JSON)"
    "crates/shared/models/src/agui/events/builder.rs|skill_name|AG-UI event payload builder"
    "crates/shared/models/src/ai/execution_plan.rs|tool_name|LLM-authored execution plan; names are validated against the inventory later"
    "crates/shared/models/src/ai/template_validation.rs|tool_name|LLM-authored plan template references"
    "crates/shared/models/src/auth/claims.rs|jti|JwtClaims wire claim, typed after signature verification"
    "crates/domain/oauth/src/services/validation/id_jag.rs|jti|ID-JAG assertion wire claim (RFC draft), replay-keyed verbatim"
    "crates/shared/models/src/api/cloud/usage.rs|agent_name|cloud API response field deserialised verbatim"
    "crates/entry/api/src/services/gateway/captures.rs|tool_name|provider wire tool_use name captured verbatim for audit"
    "crates/infra/security/src/authz/audit/repository.rs|tool_name|governance_decisions.tool_name mixes tool names, entity ids and a merge label"
    "crates/app/scheduler/src/repository/otlp/records.rs|tool_name|governance_decisions.tool_name read back for export (same mixed column)"
    "crates/shared/identifiers/src/actor.rs|tool_name|Actor::from_tool_name parses an external tool-name string"
    "crates/shared/models/src/modules/api_paths.rs|server_name agent_name|URL builders over path segments"
    "crates/shared/client/src/client/mod.rs|agent_name|HTTP client path argument for a remote agent card"
    "crates/shared/provider-contracts/src/content_data.rs|content_id|extension contract: content slug handed to third-party providers"
    "crates/shared/provider-contracts/src/frontmatter.rs|content_id|extension contract: content slug handed to third-party providers"
    "crates/infra/logging/src/services/cli/banners.rs|service_name profile_name|terminal display arguments"
    "crates/app/generator/src/error/mod.rs|provider_id|page-data provider registry key, not the gateway ProviderId"
)

# Derived names whose call sites still carry the raw type: `name|reason`. The
# name is skipped by the scan until its sites convert; an entry that no longer
# matches any line is stale and fails the gate, so the list only shrinks.
PENDING=(
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

PENDING_NAMES=$(for row in "${PENDING[@]}"; do printf '%s\n' "${row%%|*}"; done | rg -v '^$')

NAMES=$(printf '%s\n%s\n' "$DERIVED" "$TABLE_NAMES" | rg -v '^$' | sort -u)
# Why: given an empty `-f` pattern file, `rg -v` prints nothing on ripgrep 14
# (Ubuntu noble) but every line on ripgrep 15, so the exclusion runs only when
# a name is pending.
if [ -n "$PENDING_NAMES" ]; then
    NAMES=$(printf '%s\n' "$NAMES" | rg -v -x -F -f <(printf '%s\n' "$PENDING_NAMES"))
fi
NAME_COUNT=$(printf '%s\n' "$NAMES" | rg -c -v '^$')
ALT=$(printf '%s\n' "$NAMES" | rg -v '^$' | paste -sd '|' -)
if [ -z "$ALT" ] || [ "${NAME_COUNT:-0}" -eq 0 ]; then
    echo "lint-raw-ids: no forbidden names derived from ${IDS_DIR}; refusing to scan with an empty name set" >&2
    exit 2
fi

raw_pattern() {
    printf '%s' "\\b($1)\\s*:\\s*(?:Option<\\s*)?(?:&\\s*(?:'[a-z_]+\\s+)?(?:mut\\s+)?)?(?:uuid::)?(?:String|str|Uuid)\\s*(?:[,>)=;{]|\$)"
}

PATTERN=$(raw_pattern "$ALT")

# Every production layer, the facade and the bridge. Carve-outs, each with its
# reason:
#   crates/entry/api/src/routes/oauth/**      RFC 6749 / 7591 wire field names
#                                              (`client_id`, `user_id` claims)
#                                              deserialised verbatim
#   crates/domain/mcp/.../session_store.rs    MCP session records keyed by the
#                                              transport's opaque `session_id`
#                                              header string
#   crates/shared/models/src/wire/**          provider wire shapes
#                                              (Anthropic SSE `message_id`,
#                                              Gemini streaming) mirror the
#                                              upstream JSON field by name
#   crates/domain/*/src/models/rows.rs        private `query_as!` decode rows:
#                                              the macro converts columns with
#                                              `From<String>`, which checked ids
#                                              do not implement; each row maps
#                                              to its typed model via `X::new`
SEARCH_DIRS=(crates/shared crates/infra crates/domain crates/app crates/entry systemprompt/src bin/bridge/src)

# Why: `.gitignore` ignores every `audit/` directory and re-includes the source
# ones by negation; ripgrep 14 (Ubuntu noble) does not honour that negation, so
# the scan reads no ignore files and relies on the explicit exclusions below.
scan() {
    rg -n --no-heading --color=never --no-ignore \
        -g '*.rs' \
        -g '!crates/tests/**' \
        -g '!**/target/**' \
        -g '!**/.sqlx/**' \
        -g '!crates/entry/api/src/routes/oauth/**' \
        -g '!crates/domain/mcp/src/middleware/session_handler/session_store.rs' \
        -g '!crates/shared/models/src/wire/**' \
        -g '!crates/domain/*/src/models/rows.rs' \
        -e "$1" \
        "${SEARCH_DIRS[@]}" 2>/dev/null || true
}

RAW=$(scan "$PATTERN")

boundary_names() {
    local file="$1" entry
    for entry in "${BOUNDARY[@]}"; do
        if [ "${entry%%|*}" = "$file" ]; then
            entry="${entry#*|}"
            printf '%s\n' "${entry%%|*}"
        fi
    done
}

MATCHES=""
USED=""
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
    name=$(printf '%s' "$content" | rg -o -r '$1' "$PATTERN" | head -n 1)
    allowed=$(boundary_names "$file")
    if [ -n "$allowed" ] && printf ' %s ' $allowed | rg -q -F " $name "; then
        USED+="${file}|${name}"$'\n'
        continue
    fi
    MATCHES+="${file}:${lineno}:${content}"$'\n'
done <<< "$RAW"

STALE=""
for entry in "${BOUNDARY[@]}"; do
    file="${entry%%|*}"
    rest="${entry#*|}"
    for name in ${rest%%|*}; do
        if ! printf '%s' "$USED" | rg -q -x -F "${file}|${name}"; then
            STALE+="  ${file}|${name}"$'\n'
        fi
    done
done

while IFS= read -r name; do
    [ -z "$name" ] && continue
    if [ -z "$(scan "$(raw_pattern "$name")")" ]; then
        STALE+="  pending|${name}"$'\n'
    fi
done <<< "$PENDING_NAMES"

if [ "$REPORT" -eq 1 ]; then
    total=$(printf '%s' "$MATCHES" | rg -c '' || true)
    boundary=$(printf '%s' "$USED" | rg -c '' || true)
    echo "lint-raw-ids report: ${NAME_COUNT} forbidden names (derived from ${IDS_DIR} + name table), ${total:-0} offending lines, ${boundary:-0} boundary-value lines"
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

status=0
if [ -n "$STALE" ]; then
    echo "lint-raw-ids: stale boundary entries (no matching line; remove them):"
    printf '%s' "$STALE"
    status=1
fi

if [ -n "$MATCHES" ]; then
    echo "lint-raw-ids: raw String/&str/Uuid used for typed-ID field or argument names:"
    echo ""
    printf '%s' "$MATCHES"
    exit 1
fi

if [ "$status" -eq 0 ]; then
    echo "lint-raw-ids: OK (no raw ID fields or arguments found; ${NAME_COUNT} names checked)"
fi
exit "$status"
