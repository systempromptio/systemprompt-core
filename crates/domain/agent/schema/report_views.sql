-- The `analytics` report views over the `agent` member's tables: the read
-- contract the analytics reports query, contributed by the member that owns
-- the source tables, so a composition without that member has neither the
-- tables nor their views. They follow the rules of the analytics member's
-- report_views.sql: each names its columns, rows of a user whose status is
-- `deleted` are invisible, and each is replaced in place (a change that
-- drops, renames or reorders a column ships a migration that drops the view
-- first).

CREATE OR REPLACE VIEW report_agent_tasks AS
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

CREATE OR REPLACE VIEW report_task_messages AS
SELECT
    src.id, src.task_id, src.message_id, src.client_message_id, src.role,
    src.context_id, src.user_id, src.session_id, src.trace_id,
    src.sequence_number, src.created_at, src.updated_at, src.metadata,
    src.reference_task_ids
FROM task_messages src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_user_contexts AS
SELECT
    src.context_id, src.user_id, src.session_id, src.name, src.kind,
    src.created_at, src.updated_at
FROM user_contexts src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE OR REPLACE VIEW report_agent_task_hourly AS
SELECT
    src.bucket, src.user_id, src.agent_name, src.status, src.task_count,
    src.exec_sum_ms, src.exec_n, src.last_started_at
FROM agent_task_rollup_hourly src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);
