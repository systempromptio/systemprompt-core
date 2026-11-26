-- The `analytics` report views over the `ai` member's tables: the read
-- contract the analytics reports query, contributed by the member that owns
-- the source tables, so a composition without that member has neither the
-- tables nor their views. They follow the rules of the analytics member's
-- report_views.sql: each names its columns, rows of a user whose status is
-- `deleted` are invisible, and each is replaced in place (a change that
-- drops, renames or reorders a column ships a migration that drops the view
-- first).

CREATE OR REPLACE VIEW report_ai_requests AS
SELECT
    src.id, src.request_id, src.user_id, src.session_id, src.task_id,
    src.context_id, src.gateway_conversation_id, src.client_session_id,
    src.provider_request_id, src.trace_id, src.mcp_execution_id,
    src.provider, src.served_provider, src.model, src.requested_model,
    src.system_prompt_override, src.route_match, src.temperature, src.top_p,
    src.max_tokens, src.stop_sequences, src.tokens_used, src.input_tokens,
    src.output_tokens, src.cost_microdollars, src.latency_ms,
    src.upstream_latency_ms, src.finish_reason, src.cache_hit,
    src.cache_read_tokens, src.cache_creation_tokens, src.reasoning_tokens,
    src.is_streaming, src.status, src.error_message,
    src.accounting_failed_at, src.accounting_error, src.actor_kind,
    src.actor_id, src.traffic, src.request_kind, src.client_kind,
    src.wire_protocol, src.client_attestation, src.message_count,
    src.instance_id, src.created_at, src.updated_at, src.completed_at
FROM ai_requests src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

-- The hourly usage rollups, behind the same deleted-user filter: a rollup
-- keeps the user as a dimension so the filter applies to it unchanged, and a
-- user deleted or restored later needs no rebuild.
CREATE OR REPLACE VIEW report_ai_usage_hourly AS
SELECT
    src.bucket, src.user_id, src.provider, src.model, src.agent_name,
    src.traffic, src.request_count, src.tokens, src.input_tokens,
    src.output_tokens, src.reasoning_tokens, src.cache_read_tokens,
    src.cache_creation_tokens, src.cost_microdollars, src.latency_sum_ms,
    src.latency_n, src.cache_hits, src.user_key_request_count,
    src.user_key_tokens, src.user_key_cost_microdollars,
    src.user_subscription_request_count, src.user_subscription_tokens,
    src.user_subscription_cost_microdollars
FROM ai_usage_rollup_hourly src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_ai_context_hourly AS
SELECT
    src.bucket, src.user_id, src.context_id, src.model, src.agent_name,
    src.traffic, src.status, src.request_count, src.tokens,
    src.cost_microdollars, src.last_activity
FROM ai_context_rollup_hourly src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

-- A request's attribution scopes (dimension/value pairs), keyed by
-- report_ai_requests.id; visible while the request's user is.
CREATE OR REPLACE VIEW report_ai_request_attributions AS
SELECT src.request_id, src.dimension, src.value, src.source
FROM ai_request_attributions src
JOIN ai_requests r ON r.id = src.request_id
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = r.user_id AND d.status = 'deleted'
);

-- The tool calls a request's model emitted, keyed by report_ai_requests.id.
-- The call's arguments and result payload stay in the source table.
CREATE OR REPLACE VIEW report_ai_request_tool_calls AS
SELECT
    src.id, src.request_id, src.tool_name, src.mcp_execution_id,
    src.ai_tool_call_id, src.sequence_number, src.args_validity,
    src.args_error_path, src.created_at, src.updated_at
FROM ai_request_tool_calls src
JOIN ai_requests r ON r.id = src.request_id
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = r.user_id AND d.status = 'deleted'
);
