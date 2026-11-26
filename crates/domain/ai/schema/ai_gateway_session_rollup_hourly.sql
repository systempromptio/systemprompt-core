-- Gateway sessions rolled up per hour at write time: one row per (UTC hour,
-- session, user) over the AI requests that belong to no task, carrying the
-- request count, the messages those requests hold, and the earliest and
-- latest `created_at` in the row. It is the fact behind the gateway session
-- list of the conversations report, which reads the hours in its window
-- instead of grouping the request ledger by session on every load.
--
-- The writer is a set of statement triggers on `ai_requests`: an insert adds
-- its rows, a delete subtracts them, and an update subtracts the old version
-- and adds the new one for rows whose rolled-up columns changed (a request
-- that gains a task leaves the rollup; `message_count` grows as messages are
-- stored). A row that falls to zero requests is removed; a surviving row that
-- lost requests has its `first_at` and `last_at` re-read for its own
-- session-hour. `refresh_ai_gateway_session_rollup_hourly(keys)` rebuilds
-- whole hours (keys are `YYYY-MM-DDTHH:00:00Z`) from `ai_requests`; migration
-- 060 queues every hour that predates the table and the `backfill_rollups`
-- job drains them.

CREATE TABLE IF NOT EXISTS ai_gateway_session_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    session_id VARCHAR(255) NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    request_count BIGINT NOT NULL DEFAULT 0,
    message_count BIGINT NOT NULL DEFAULT 0,
    first_at TIMESTAMPTZ,
    last_at TIMESTAMPTZ,
    PRIMARY KEY (bucket, session_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_ai_gateway_session_rollup_hourly_user_bucket
    ON ai_gateway_session_rollup_hourly(user_id, bucket);

CREATE OR REPLACE FUNCTION ai_gateway_session_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO ai_gateway_session_rollup_hourly AS r (
        bucket, session_id, user_id, request_count, message_count, first_at, last_at)
    SELECT date_trunc(''hour'', s.created_at, ''UTC''), s.session_id, s.user_id,
           SUM(s.sign), SUM(s.sign * s.message_count),
           MIN(s.created_at) FILTER (WHERE s.sign > 0),
           MAX(s.created_at) FILTER (WHERE s.sign > 0)
    FROM (%s) s
    WHERE s.task_id IS NULL AND s.session_id IS NOT NULL
    GROUP BY 1, 2, 3
    ORDER BY 1, 2, 3
    ON CONFLICT (bucket, session_id, user_id) DO UPDATE SET
        request_count = r.request_count + EXCLUDED.request_count,
        message_count = r.message_count + EXCLUDED.message_count,
        first_at = LEAST(r.first_at, EXCLUDED.first_at),
        last_at = GREATEST(r.last_at, EXCLUDED.last_at)
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_ai_gateway_session_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM ai_gateway_session_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE ai_gateway_session_rollup_upsert_sql(
        'SELECT a.*, 1 AS sign FROM ai_requests a JOIN unnest($1) h(b) '
        'ON a.created_at >= h.b AND a.created_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION ai_gateway_session_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.created_at, n.session_id, n.user_id, n.task_id, n.message_count) '
        'IS DISTINCT FROM (o.created_at, o.session_id, o.user_id, o.task_id, o.message_count)';
    departed TEXT := 'SELECT o.created_at, o.session_id FROM old_rows o';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE ai_gateway_session_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE ai_gateway_session_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.id = n.id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n ON n.id = o.id WHERE '
            || changed);
        departed := departed || ' JOIN new_rows n ON n.id = o.id WHERE ' || changed;
    ELSE
        EXECUTE ai_gateway_session_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM ai_gateway_session_rollup_hourly
    WHERE request_count = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.created_at, 'UTC') FROM old_rows o));
    EXECUTE format('
    UPDATE ai_gateway_session_rollup_hourly r
    SET (first_at, last_at) = (
        SELECT MIN(a.created_at), MAX(a.created_at)
        FROM ai_requests a
        WHERE a.session_id = r.session_id AND a.user_id = r.user_id AND a.task_id IS NULL
          AND a.created_at >= r.bucket AND a.created_at < r.bucket + interval ''1 hour'')
    WHERE (r.bucket, r.session_id) IN (
        SELECT DISTINCT date_trunc(''hour'', d.created_at, ''UTC''), d.session_id FROM (%s) d
        WHERE d.session_id IS NOT NULL)',
        departed);
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS ai_gateway_session_rollup_insert ON ai_requests;
CREATE TRIGGER ai_gateway_session_rollup_insert
    AFTER INSERT ON ai_requests
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_gateway_session_rollup_apply();

DROP TRIGGER IF EXISTS ai_gateway_session_rollup_update ON ai_requests;
CREATE TRIGGER ai_gateway_session_rollup_update
    AFTER UPDATE ON ai_requests
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_gateway_session_rollup_apply();

DROP TRIGGER IF EXISTS ai_gateway_session_rollup_delete ON ai_requests;
CREATE TRIGGER ai_gateway_session_rollup_delete
    AFTER DELETE ON ai_requests
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_gateway_session_rollup_apply();
