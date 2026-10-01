#!/usr/bin/env bash
set -uo pipefail

# Field names whose typed ID exists in systemprompt_identifiers. The second
# block is every such name with no raw site left; a name joins the list when
# its last raw `String` site is converted (instance_id, plugin_id, trace_id,
# mcp_execution_id, challenge_id, … still have raw sites).
BANNED_NAMES=(
    user_id agent_id task_id tenant_id context_id session_id file_id skill_id
    client_id artifact_id message_id role_id hook_id execution_step_id
    content_id source_id call_id requested_by approver_id
    access_token_id ai_gateway_policy_id ai_quota_bucket_id ai_request_id
    ai_safety_finding_id api_key_id client_session_id config_id connection_id
    consumer_installation_id department_id device_cert_id device_id
    distribution_id engagement_event_id event_outbox_id external_agent_id
    installation_receipt_id installation_session_binding_id inventory_entry_id
    link_click_id log_id managed_reconciliation_id managed_resource_id
    managed_source_id marketplace_id native_session_id policy_id price_id
    publication_review_id resource_invocation_id resource_revision_id route_id
    rule_id scheduled_job_id secret_pattern_id section_id slack_channel_id
    slack_user_id slack_workspace_id source_snapshot_id tag_id
    teams_conversation_id teams_tenant_id teams_user_id token_id
    webhook_endpoint_id withdrawal_proposal_id
)
BANNED="($(IFS='|'; echo "${BANNED_NAMES[*]}"))"

PATTERN="(\bpub\s+)?\b${BANNED}\s*:\s*(Option<)?&?(\s)?(mut\s+)?(String|str)\b"

# Every production layer, the facade and the bridge. Carve-outs, each with its
# reason (a stale carve-out — one matching no line — is removed, not kept):
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
SEARCH_DIRS=(crates/shared crates/infra crates/domain crates/app crates/entry systemprompt/src bin/bridge/src)

if command -v rg >/dev/null 2>&1; then
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
else
    RAW=$(grep -rnE \
        --include='*.rs' \
        --exclude-dir=target \
        --exclude-dir=.sqlx \
        --exclude-dir=tests \
        --exclude-dir=oauth \
        "$PATTERN" \
        "${SEARCH_DIRS[@]}" 2>/dev/null || true)
fi

MATCHES=""
while IFS= read -r line; do
    [ -z "$line" ] && continue
    file="${line%%:*}"
    rest="${line#*:}"
    lineno="${rest%%:*}"
    content="${rest#*:}"

    case "$file" in
        crates/entry/api/src/routes/oauth/*) continue ;;
        crates/domain/mcp/src/middleware/session_handler/session_store.rs) continue ;;
        crates/shared/models/src/wire/*) continue ;;
        crates/tests/*) continue ;;
        */target/*|*/.sqlx/*) continue ;;
    esac

    MATCHES+="${file}:${lineno}:${content}"$'\n'
done <<< "$RAW"

if [ -z "$MATCHES" ]; then
    echo "lint-raw-ids: OK (no raw ID fields found)"
    exit 0
fi

echo "lint-raw-ids: raw String/&str used for typed-ID field names:"
echo ""
printf '%s' "$MATCHES"
exit 1
