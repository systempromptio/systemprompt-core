-- The read contract of the `analytics` reports: one view per source table,
-- so analytics queries only report relations while the rows stay where their
-- owners write them. The views here read tables the kernel always links; a
-- view over an optional member's table (`ai`, `agent`, `mcp`, `content`)
-- is contributed by that member in its own report_views.sql. Nothing is copied — every report is current as of
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
-- Replaced in place, so a boot takes no drop and a consumer view built on a
-- report survives it. CREATE OR REPLACE can append a column but cannot drop,
-- rename or reorder one: a change that does ships a migration that runs
-- `DROP VIEW IF EXISTS report_<name>;` first, and the dependent phase, which
-- runs after migrations, lays the new definition down.

CREATE OR REPLACE VIEW report_users AS
SELECT
    src.id, src.name, src.email, src.full_name, src.display_name,
    src.status, src.email_verified, src.roles, src.is_bot, src.is_scanner,
    src.avatar_url, src.created_at, src.updated_at
FROM users src
WHERE src.status <> 'deleted';

CREATE OR REPLACE VIEW report_user_sessions AS
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

CREATE OR REPLACE VIEW report_clean_traffic AS
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

CREATE OR REPLACE VIEW report_engaged_traffic AS
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

CREATE OR REPLACE VIEW report_bot_sessions AS
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

CREATE OR REPLACE VIEW report_analytics_events AS
SELECT
    src.id, src.user_id, src.session_id, src.context_id,
    src.gateway_conversation_id, src.provider_request_id, src.event_type,
    src.event_category, src.severity, src.endpoint, src.error_code,
    src.response_time_ms, src.agent_id, src.task_id, src.message,
    src.metadata, src.event_data, src."timestamp", src.target_url, src.is_external
FROM analytics_events src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_link_click_hourly AS
SELECT src.bucket, src.user_id, src.endpoint, src.target_url, src.is_external, src.clicks
FROM analytics_link_click_rollup_hourly src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_content_engagement_hourly AS
SELECT src.bucket, src.content_id, src.session_id, src.views, src.time_on_page_sum_ms
FROM content_engagement_rollup_hourly src
JOIN report_clean_traffic c ON c.session_id = src.session_id
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

-- `person` is a user-level filter: `users` carries only `is_bot` and
-- `is_scanner`, not the session predicate `is_human_traffic` states over the
-- crawler and behavioural flags.
CREATE OR REPLACE VIEW report_page_view_hourly AS
SELECT
    src.bucket, src.user_id, src.endpoint, src.event_category, src.views,
    COALESCE(u.is_bot = FALSE AND u.is_scanner = FALSE, FALSE) AS person
FROM analytics_page_view_rollup_hourly src
LEFT JOIN users u ON u.id = src.user_id
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_governance_decisions AS
SELECT
    src.id, src.user_id, src.session_id, src.tool_name, src.agent_id,
    src.agent_scope, src.decision, src.policy, src.reason, src.plugin_id,
    src.actor_kind, src.actor_id, src.context_id, src.task_id, src.trace_id,
    src.client_id, src.tool_use_id, src.created_at
FROM governance_decisions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);
