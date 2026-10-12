-- Tool executions and agent tasks, rolled up per hour at write time (store,
-- don't compute), beside core ai's `ai_usage_rollup_hourly`. The analytics
-- reports read these through `report_tool_usage_hourly` and
-- `report_agent_task_hourly` instead of re-aggregating the ledger on every
-- load. Averages are `*_sum / *_n`; percentiles and top-N error strings keep
-- reading the ledger for their window.
--
--   tool_usage_rollup_hourly  (UTC hour of created_at, user, tool, server,
--                              agent of the task, status)
--   agent_task_rollup_hourly  (UTC hour of started_at, user, agent, status);
--                             a task with no start time is not counted
--
-- Dimensions are NOT NULL so they can form the primary key; an absent user or
-- agent is ''. `last_at` / `last_started_at` are the latest timestamp added
-- to the row: a subtraction never lowers them, so after a delete they are an
-- upper bound until the hour is refreshed.
--
-- Statement triggers keep both current: an insert adds its rows, a delete
-- subtracts them, an update subtracts the old version and adds the new one
-- for rows whose rolled-up columns changed; a row that falls to zero is
-- removed. `refresh_tool_usage_rollup_hourly(keys)` and
-- `refresh_agent_task_rollup_hourly(keys)` rebuild whole hours (keys are
-- `YYYY-MM-DDTHH:00:00Z`) from the ledger; migration 018 queues every hour
-- that predates the tables and the `backfill_rollups` job drains them.

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

CREATE OR REPLACE FUNCTION tool_usage_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO tool_usage_rollup_hourly AS r (
        bucket, user_id, tool_name, server_name, agent_name, status, execution_count,
        exec_sum_ms, exec_n, last_at)
    SELECT date_trunc(''hour'', s.created_at, ''UTC''), s.user_id, s.tool_name, s.server_name,
           COALESCE(t.agent_name, ''''), s.status, SUM(s.sign),
           SUM(s.sign * COALESCE(s.execution_time_ms, 0)),
           COALESCE(SUM(s.sign) FILTER (WHERE s.execution_time_ms IS NOT NULL), 0),
           MAX(s.created_at) FILTER (WHERE s.sign > 0)
    FROM (%s) s
    LEFT JOIN agent_tasks t ON t.task_id = s.task_id
    GROUP BY 1, 2, 3, 4, 5, 6
    ORDER BY 1, 2, 3, 4, 5, 6
    ON CONFLICT (bucket, user_id, tool_name, server_name, agent_name, status) DO UPDATE SET
        execution_count = r.execution_count + EXCLUDED.execution_count,
        exec_sum_ms = r.exec_sum_ms + EXCLUDED.exec_sum_ms,
        exec_n = r.exec_n + EXCLUDED.exec_n,
        last_at = GREATEST(r.last_at, EXCLUDED.last_at)
', source)
$$;

CREATE OR REPLACE FUNCTION agent_task_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO agent_task_rollup_hourly AS r (
        bucket, user_id, agent_name, status, task_count, exec_sum_ms, exec_n, last_started_at)
    SELECT date_trunc(''hour'', s.started_at, ''UTC''), COALESCE(s.user_id, ''''),
           COALESCE(s.agent_name, ''''), s.status, SUM(s.sign),
           SUM(s.sign * COALESCE(s.execution_time_ms, 0)),
           COALESCE(SUM(s.sign) FILTER (WHERE s.execution_time_ms IS NOT NULL), 0),
           MAX(s.started_at) FILTER (WHERE s.sign > 0)
    FROM (%s) s
    WHERE s.started_at IS NOT NULL
    GROUP BY 1, 2, 3, 4
    ORDER BY 1, 2, 3, 4
    ON CONFLICT (bucket, user_id, agent_name, status) DO UPDATE SET
        task_count = r.task_count + EXCLUDED.task_count,
        exec_sum_ms = r.exec_sum_ms + EXCLUDED.exec_sum_ms,
        exec_n = r.exec_n + EXCLUDED.exec_n,
        last_started_at = GREATEST(r.last_started_at, EXCLUDED.last_started_at)
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_tool_usage_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM tool_usage_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE tool_usage_rollup_upsert_sql(
        'SELECT e.*, 1 AS sign FROM mcp_tool_executions e JOIN unnest($1) h(b) '
        'ON e.created_at >= h.b AND e.created_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION refresh_agent_task_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM agent_task_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE agent_task_rollup_upsert_sql(
        'SELECT a.*, 1 AS sign FROM agent_tasks a JOIN unnest($1) h(b) '
        'ON a.started_at >= h.b AND a.started_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION tool_usage_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.created_at, n.user_id, n.tool_name, n.server_name, n.task_id, '
        'n.status, n.execution_time_ms) IS DISTINCT FROM (o.created_at, o.user_id, o.tool_name, '
        'o.server_name, o.task_id, o.status, o.execution_time_ms)';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE tool_usage_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE tool_usage_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o '
            'ON o.mcp_execution_id = n.mcp_execution_id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n '
            'ON n.mcp_execution_id = o.mcp_execution_id WHERE ' || changed);
    ELSE
        EXECUTE tool_usage_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM tool_usage_rollup_hourly
    WHERE execution_count = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.created_at, 'UTC') FROM old_rows o));
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION agent_task_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.started_at, n.user_id, n.agent_name, n.status, n.execution_time_ms) '
        'IS DISTINCT FROM (o.started_at, o.user_id, o.agent_name, o.status, o.execution_time_ms)';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE agent_task_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE agent_task_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.task_id = n.task_id WHERE '
            || changed || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n '
            'ON n.task_id = o.task_id WHERE ' || changed);
    ELSE
        EXECUTE agent_task_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM agent_task_rollup_hourly
    WHERE task_count = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.started_at, 'UTC') FROM old_rows o
                             WHERE o.started_at IS NOT NULL));
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS tool_usage_rollup_insert ON mcp_tool_executions;
CREATE TRIGGER tool_usage_rollup_insert
    AFTER INSERT ON mcp_tool_executions
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION tool_usage_rollup_apply();

DROP TRIGGER IF EXISTS tool_usage_rollup_update ON mcp_tool_executions;
CREATE TRIGGER tool_usage_rollup_update
    AFTER UPDATE ON mcp_tool_executions
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION tool_usage_rollup_apply();

DROP TRIGGER IF EXISTS tool_usage_rollup_delete ON mcp_tool_executions;
CREATE TRIGGER tool_usage_rollup_delete
    AFTER DELETE ON mcp_tool_executions
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION tool_usage_rollup_apply();

DROP TRIGGER IF EXISTS agent_task_rollup_insert ON agent_tasks;
CREATE TRIGGER agent_task_rollup_insert
    AFTER INSERT ON agent_tasks
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION agent_task_rollup_apply();

DROP TRIGGER IF EXISTS agent_task_rollup_update ON agent_tasks;
CREATE TRIGGER agent_task_rollup_update
    AFTER UPDATE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION agent_task_rollup_apply();

DROP TRIGGER IF EXISTS agent_task_rollup_delete ON agent_tasks;
CREATE TRIGGER agent_task_rollup_delete
    AFTER DELETE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION agent_task_rollup_apply();
