-- Agent tasks rolled up per hour at write time (store, don't compute). The
-- analytics reports read it through `report_agent_task_hourly` instead of
-- re-aggregating the ledger on every load. Averages are `*_sum / *_n`.
--
--   agent_task_rollup_hourly  (UTC hour of started_at, user, agent, status);
--                             a task with no start time is not counted
--
-- Dimensions are NOT NULL so they can form the primary key; an absent user or
-- agent is ''. `last_started_at` is the latest timestamp added to the row: a
-- subtraction never lowers it, so after a delete it is an upper bound until
-- the hour is refreshed.
--
-- Statement triggers on agent_tasks keep it current the same way as
-- `tool_usage_rollup_hourly`; `refresh_agent_task_rollup_hourly(keys)`
-- rebuilds whole hours from the ledger, queued by migration 018 and drained
-- by the `backfill_rollups` job.

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
