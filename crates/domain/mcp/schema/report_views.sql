-- The `analytics` report views over the `mcp` member's tables: the read
-- contract the analytics reports query, contributed by the member that owns
-- the source tables, so a composition without that member has neither the
-- tables nor their views. They follow the rules of the analytics member's
-- report_views.sql: each names its columns, rows of a user whose status is
-- `deleted` are invisible, and each is replaced in place (a change that
-- drops, renames or reorders a column ships a migration that drops the view
-- first).

CREATE OR REPLACE VIEW report_mcp_tool_executions AS
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

CREATE OR REPLACE VIEW report_tool_usage_hourly AS
SELECT
    src.bucket, src.user_id, src.tool_name, src.server_name, src.agent_name,
    src.status, src.execution_count, src.exec_sum_ms, src.exec_n, src.last_at
FROM tool_usage_rollup_hourly src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);
