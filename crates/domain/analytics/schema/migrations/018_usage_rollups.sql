-- Tool executions and agent tasks rolled up per hour
-- (schema/usage_rollups.sql). The tables are the base schema's text; the
-- hours that predate them are queued, keys only, for
-- `refresh_tool_usage_rollup_hourly` and `refresh_agent_task_rollup_hourly`,
-- which the `backfill_rollups` job drains. Rows written after this migration
-- are rolled up by the statement triggers, and a queued hour is rebuilt whole
-- from the ledger.
CREATE TABLE IF NOT EXISTS tool_usage_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    tool_name VARCHAR(255) NOT NULL,
    server_name VARCHAR(255) NOT NULL,
    agent_name TEXT NOT NULL,
    status VARCHAR(255) NOT NULL,
    execution_count BIGINT NOT NULL DEFAULT 0,
    exec_sum_ms BIGINT NOT NULL DEFAULT 0,
    exec_n BIGINT NOT NULL DEFAULT 0,
    last_at TIMESTAMPTZ,
    PRIMARY KEY (bucket, user_id, tool_name, server_name, agent_name, status)
);

CREATE TABLE IF NOT EXISTS agent_task_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    status TEXT NOT NULL,
    task_count BIGINT NOT NULL DEFAULT 0,
    exec_sum_ms BIGINT NOT NULL DEFAULT 0,
    exec_n BIGINT NOT NULL DEFAULT 0,
    last_started_at TIMESTAMPTZ,
    PRIMARY KEY (bucket, user_id, agent_name, status)
);

INSERT INTO projection_backlog (projection, key)
SELECT 'tool_usage_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM mcp_tool_executions
ON CONFLICT DO NOTHING;

INSERT INTO projection_backlog (projection, key)
SELECT 'agent_task_rollup_hourly',
       to_char(date_trunc('hour', started_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM agent_tasks
WHERE started_at IS NOT NULL
ON CONFLICT DO NOTHING;
