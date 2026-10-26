-- AI requests rolled up per hour and conversation at write time: one row per
-- (UTC hour, user, context, model, agent, traffic, status) carrying the
-- request count, tokens, cost and the latest `created_at` in the row. It is
-- the fact behind the per-conversation analytics a window asks for — distinct
-- conversations per user, model or agent, the most recent conversations, and
-- per-status spend for the anomaly scan — which the provider-keyed
-- `ai_usage_rollup_hourly` cannot answer. A distinct count over a window reads
-- these rows (at most one per conversation-hour-dimension), never the ledger.
--
-- Dimensions are NOT NULL so they can form the primary key: a request with no
-- model or agent is stored under ''. `agent_name` is the agent of the
-- request's task, resolved when the row is written.
--
-- The writer is a set of statement triggers on `ai_requests`: an insert adds
-- its rows, a delete subtracts them, and an update subtracts the old version
-- and adds the new one for rows whose rolled-up columns changed. A row that
-- falls to zero requests is removed; a surviving row that lost requests has
-- its `last_activity` re-read for its own conversation-hour.
-- `refresh_ai_context_rollup_hourly(keys)` rebuilds whole hours (keys are
-- `YYYY-MM-DDTHH:00:00Z`) from `ai_requests`; migration 054 queues every hour
-- that predates the table and the `backfill_rollups` job drains them.

CREATE TABLE IF NOT EXISTS ai_context_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    context_id VARCHAR(255) NOT NULL,
    model TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    traffic TEXT NOT NULL
        CONSTRAINT ai_context_rollup_hourly_traffic_check CHECK (traffic IN ('production', 'evaluation')),
    status VARCHAR(255) NOT NULL,
    request_count BIGINT NOT NULL DEFAULT 0,
    tokens BIGINT NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    last_activity TIMESTAMPTZ,
    PRIMARY KEY (bucket, user_id, context_id, model, agent_name, traffic, status)
);
CREATE INDEX IF NOT EXISTS idx_ai_context_rollup_hourly_user_bucket
    ON ai_context_rollup_hourly(user_id, bucket);
CREATE INDEX IF NOT EXISTS idx_ai_context_rollup_hourly_context
    ON ai_context_rollup_hourly(context_id, last_activity);

CREATE OR REPLACE FUNCTION ai_context_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO ai_context_rollup_hourly AS r (
        bucket, user_id, context_id, model, agent_name, traffic, status,
        request_count, tokens, cost_microdollars, last_activity)
    SELECT date_trunc(''hour'', s.created_at, ''UTC''), s.user_id, s.context_id,
           COALESCE(s.model, ''''), COALESCE(t.agent_name, ''''), s.traffic, s.status,
           SUM(s.sign), SUM(s.sign * COALESCE(s.tokens_used, 0)),
           SUM(s.sign * s.cost_microdollars),
           MAX(s.created_at) FILTER (WHERE s.sign > 0)
    FROM (%s) s
    LEFT JOIN agent_tasks t ON t.task_id = s.task_id
    GROUP BY 1, 2, 3, 4, 5, 6, 7
    ORDER BY 1, 2, 3, 4, 5, 6, 7
    ON CONFLICT (bucket, user_id, context_id, model, agent_name, traffic, status) DO UPDATE SET
        request_count = r.request_count + EXCLUDED.request_count,
        tokens = r.tokens + EXCLUDED.tokens,
        cost_microdollars = r.cost_microdollars + EXCLUDED.cost_microdollars,
        last_activity = GREATEST(r.last_activity, EXCLUDED.last_activity)
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_ai_context_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM ai_context_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE ai_context_rollup_upsert_sql(
        'SELECT a.*, 1 AS sign FROM ai_requests a JOIN unnest($1) h(b) '
        'ON a.created_at >= h.b AND a.created_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION ai_context_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.created_at, n.user_id, n.context_id, n.model, n.task_id, '
        'n.traffic, n.status, n.tokens_used, n.cost_microdollars) IS DISTINCT FROM '
        '(o.created_at, o.user_id, o.context_id, o.model, o.task_id, o.traffic, o.status, '
        'o.tokens_used, o.cost_microdollars)';
    departed TEXT := 'SELECT o.created_at, o.context_id FROM old_rows o';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE ai_context_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE ai_context_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.id = n.id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n ON n.id = o.id WHERE '
            || changed);
        departed := departed || ' JOIN new_rows n ON n.id = o.id WHERE ' || changed;
    ELSE
        EXECUTE ai_context_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM ai_context_rollup_hourly
    WHERE request_count = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.created_at, 'UTC') FROM old_rows o));
    EXECUTE format('
    UPDATE ai_context_rollup_hourly r
    SET last_activity = (
        SELECT MAX(a.created_at)
        FROM ai_requests a
        LEFT JOIN agent_tasks t ON t.task_id = a.task_id
        WHERE a.context_id = r.context_id
          AND a.created_at >= r.bucket AND a.created_at < r.bucket + interval ''1 hour''
          AND a.user_id = r.user_id AND COALESCE(a.model, '''') = r.model
          AND COALESCE(t.agent_name, '''') = r.agent_name
          AND a.traffic = r.traffic AND a.status = r.status)
    WHERE (r.bucket, r.context_id) IN (
        SELECT DISTINCT date_trunc(''hour'', d.created_at, ''UTC''), d.context_id FROM (%s) d)',
        departed);
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS ai_context_rollup_insert ON ai_requests;
CREATE TRIGGER ai_context_rollup_insert
    AFTER INSERT ON ai_requests
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_context_rollup_apply();

DROP TRIGGER IF EXISTS ai_context_rollup_update ON ai_requests;
CREATE TRIGGER ai_context_rollup_update
    AFTER UPDATE ON ai_requests
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_context_rollup_apply();

DROP TRIGGER IF EXISTS ai_context_rollup_delete ON ai_requests;
CREATE TRIGGER ai_context_rollup_delete
    AFTER DELETE ON ai_requests
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_context_rollup_apply();
