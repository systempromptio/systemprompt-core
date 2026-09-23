-- The read contract of the `analytics` reports: one view per source table,
-- so analytics queries only relations it declares while the rows stay where
-- their owners write them. Nothing is copied — every report is current as of
-- its query. Privacy is enforced here, once: rows belonging to a user whose
-- status is `deleted` are invisible to every report; age limits come from
-- retention deleting source rows.
--
-- Every view names its columns. `SELECT *` would take the source table's
-- physical column order, which differs between a fresh install and an
-- upgraded database wherever a migration appended a column the declarative
-- schema places earlier (ai_requests.message_count), so the same view would
-- have two definitions. A column added to a source table is added here too.
--
-- Dropped and recreated rather than replaced: CREATE OR REPLACE cannot drop or
-- reorder a column. Views are stateless, so dropping loses nothing.

DROP VIEW IF EXISTS report_analytics_events;
DROP VIEW IF EXISTS report_markdown_content;
DROP VIEW IF EXISTS report_mcp_tool_executions;
DROP VIEW IF EXISTS report_user_contexts;
DROP VIEW IF EXISTS report_task_messages;
DROP VIEW IF EXISTS report_agent_tasks;
DROP VIEW IF EXISTS report_ai_requests;
DROP VIEW IF EXISTS report_bot_sessions;
DROP VIEW IF EXISTS report_engaged_traffic;
DROP VIEW IF EXISTS report_clean_traffic;
DROP VIEW IF EXISTS report_user_sessions;
DROP VIEW IF EXISTS report_users;

CREATE VIEW report_users AS
SELECT
    src.id, src.name, src.email, src.full_name, src.display_name,
    src.status, src.email_verified, src.roles, src.is_bot, src.is_scanner,
    src.avatar_url, src.created_at, src.updated_at
FROM users src
WHERE src.status <> 'deleted';

CREATE VIEW report_user_sessions AS
SELECT
    src.session_id, src.user_id, src.started_at, src.last_activity_at,
    src.ended_at, src.duration_seconds, src.user_type, src.converted_at,
    src.expires_at, src.client_id, src.client_type, src.request_count,
    src.avg_response_time_ms, src.success_rate, src.error_count,
    src.task_count, src.message_count, src.ai_request_count,
    src.total_tokens_used, src.total_ai_cost_microdollars, src.ip_address,
    src.user_agent, src.device_type, src.browser, src.os, src.country,
    src.region, src.city, src.preferred_locale, src.referrer_source,
    src.referrer_url, src.landing_page, src.entry_url, src.utm_source,
    src.utm_medium, src.utm_campaign, src.utm_content, src.utm_term,
    src.endpoints_accessed, src.fingerprint_hash, src.is_bot,
    src.is_ai_crawler, src.is_scanner, src.is_behavioral_bot,
    src.behavioral_bot_reason, src.behavioral_bot_score, src.session_source,
    src.revoked_at
FROM user_sessions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_clean_traffic AS
SELECT
    src.session_id, src.user_id, src.started_at, src.last_activity_at,
    src.ended_at, src.duration_seconds, src.user_type, src.converted_at,
    src.expires_at, src.client_id, src.client_type, src.request_count,
    src.avg_response_time_ms, src.success_rate, src.error_count,
    src.task_count, src.message_count, src.ai_request_count,
    src.total_tokens_used, src.total_ai_cost_microdollars, src.ip_address,
    src.user_agent, src.device_type, src.browser, src.os, src.country,
    src.region, src.city, src.preferred_locale, src.referrer_source,
    src.referrer_url, src.landing_page, src.entry_url, src.utm_source,
    src.utm_medium, src.utm_campaign, src.utm_content, src.utm_term,
    src.endpoints_accessed, src.fingerprint_hash, src.is_bot,
    src.is_ai_crawler, src.is_scanner, src.is_behavioral_bot,
    src.behavioral_bot_reason, src.behavioral_bot_score, src.session_source,
    src.revoked_at
FROM v_clean_traffic src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_engaged_traffic AS
SELECT
    src.session_id, src.user_id, src.started_at, src.last_activity_at,
    src.ended_at, src.duration_seconds, src.user_type, src.converted_at,
    src.expires_at, src.client_id, src.client_type, src.request_count,
    src.avg_response_time_ms, src.success_rate, src.error_count,
    src.task_count, src.message_count, src.ai_request_count,
    src.total_tokens_used, src.total_ai_cost_microdollars, src.ip_address,
    src.user_agent, src.device_type, src.browser, src.os, src.country,
    src.region, src.city, src.preferred_locale, src.referrer_source,
    src.referrer_url, src.landing_page, src.entry_url, src.utm_source,
    src.utm_medium, src.utm_campaign, src.utm_content, src.utm_term,
    src.endpoints_accessed, src.fingerprint_hash, src.is_bot,
    src.is_ai_crawler, src.is_scanner, src.is_behavioral_bot,
    src.behavioral_bot_reason, src.behavioral_bot_score, src.session_source,
    src.revoked_at
FROM v_engaged_traffic src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_bot_sessions AS
SELECT
    src.session_id, src.user_id, src.started_at, src.last_activity_at,
    src.ended_at, src.duration_seconds, src.user_type, src.converted_at,
    src.expires_at, src.client_id, src.client_type, src.request_count,
    src.avg_response_time_ms, src.success_rate, src.error_count,
    src.task_count, src.message_count, src.ai_request_count,
    src.total_tokens_used, src.total_ai_cost_microdollars, src.ip_address,
    src.user_agent, src.device_type, src.browser, src.os, src.country,
    src.region, src.city, src.preferred_locale, src.referrer_source,
    src.referrer_url, src.landing_page, src.entry_url, src.utm_source,
    src.utm_medium, src.utm_campaign, src.utm_content, src.utm_term,
    src.endpoints_accessed, src.fingerprint_hash, src.is_bot,
    src.is_ai_crawler, src.is_scanner, src.is_behavioral_bot,
    src.behavioral_bot_reason, src.behavioral_bot_score, src.session_source,
    src.revoked_at, src.bot_type
FROM v_bot_sessions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_ai_requests AS
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
    src.actor_id, src.synthetic, src.request_kind, src.client_kind,
    src.wire_protocol, src.client_attestation, src.message_count,
    src.instance_id, src.created_at, src.updated_at, src.completed_at
FROM ai_requests src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_agent_tasks AS
SELECT
    src.task_id, src.context_id, src.status, src.status_timestamp,
    src.user_id, src.session_id, src.trace_id, src.agent_name,
    src.started_at, src.completed_at, src.execution_time_ms,
    src.error_message, src.metadata, src.version, src.created_at,
    src.updated_at
FROM agent_tasks src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_task_messages AS
SELECT
    src.id, src.task_id, src.message_id, src.client_message_id, src.role,
    src.context_id, src.user_id, src.session_id, src.trace_id,
    src.sequence_number, src.created_at, src.updated_at, src.metadata,
    src.reference_task_ids
FROM task_messages src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_user_contexts AS
SELECT
    src.context_id, src.user_id, src.session_id, src.name, src.kind,
    src.created_at, src.updated_at
FROM user_contexts src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_mcp_tool_executions AS
SELECT
    src.mcp_execution_id, src.tool_name, src.server_name, src.started_at,
    src.completed_at, src.execution_time_ms, src.input, src.output,
    src.output_schema, src.status, src.error_message, src.user_id,
    src.session_id, src.context_id, src.task_id, src.trace_id,
    src.request_method, src.request_source, src.actor_kind, src.actor_id,
    src.ai_tool_call_id, src.source, src.correlation, src.payload_sha256,
    src.created_at
FROM mcp_tool_executions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_markdown_content AS
SELECT
    src.id, src.slug, src.locale, src.title, src.description, src.body,
    src.author, src.published_at, src.keywords, src.kind, src.image,
    src.category_id, src.source_id, src.version_hash, src.public, src.links,
    src.updated_at
FROM markdown_content src;

CREATE VIEW report_analytics_events AS
SELECT
    src.id, src.user_id, src.session_id, src.context_id,
    src.gateway_conversation_id, src.provider_request_id, src.event_type,
    src.event_category, src.severity, src.endpoint, src.error_code,
    src.response_time_ms, src.agent_id, src.task_id, src.message,
    src.metadata, src.event_data, src."timestamp"
FROM analytics_events src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);
