-- AI usage, rolled up per hour at write time (store, don't compute): one row
-- per (UTC hour, user, provider, model, agent, synthetic) carrying additive
-- measures, so a dashboard window costs the number of hours it spans instead
-- of the number of requests behind it. Averages are `*_sum / *_n`; anything
-- not additive (percentiles, distinct conversations) keeps reading
-- `ai_requests` for its window.
--
-- The `user_key_*` measures are the share of each row billed to a member's
-- own personal key (`ai_requests.billing_source = 'user_key'`); the platform
-- share is the row total minus them.
--
-- Dimensions are NOT NULL so they can form the primary key: a request with no
-- provider, model or agent is stored under ''. `agent_name` is the agent of
-- the request's task, resolved when the row is written.
--
-- The writer is a set of statement triggers on `ai_requests`: an insert adds
-- its rows, a delete subtracts them, and an update subtracts the old version
-- and adds the new one for rows whose rolled-up columns changed. A row that
-- falls to zero requests is removed. `refresh_ai_usage_rollup_hourly(keys)`
-- rebuilds whole hours (keys are `YYYY-MM-DDTHH:00:00Z`) from `ai_requests`;
-- migration 040 queues every hour that predates the table and the
-- `backfill_rollups` job drains them.

CREATE TABLE IF NOT EXISTS ai_usage_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    agent_name TEXT NOT NULL,
    synthetic BOOLEAN NOT NULL,
    request_count BIGINT NOT NULL DEFAULT 0,
    tokens BIGINT NOT NULL DEFAULT 0,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    cache_read_tokens BIGINT NOT NULL DEFAULT 0,
    cache_creation_tokens BIGINT NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    latency_sum_ms BIGINT NOT NULL DEFAULT 0,
    latency_n BIGINT NOT NULL DEFAULT 0,
    cache_hits BIGINT NOT NULL DEFAULT 0,
    user_key_request_count BIGINT NOT NULL DEFAULT 0,
    user_key_tokens BIGINT NOT NULL DEFAULT 0,
    user_key_cost_microdollars BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket, user_id, provider, model, agent_name, synthetic)
);
CREATE INDEX IF NOT EXISTS idx_ai_usage_rollup_hourly_user_bucket
    ON ai_usage_rollup_hourly(user_id, bucket);

CREATE OR REPLACE FUNCTION ai_usage_rollup_upsert_sql(source TEXT)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT format('
    INSERT INTO ai_usage_rollup_hourly AS r (
        bucket, user_id, provider, model, agent_name, synthetic, request_count, tokens,
        input_tokens, output_tokens, reasoning_tokens, cache_read_tokens,
        cache_creation_tokens, cost_microdollars, latency_sum_ms, latency_n, cache_hits,
        user_key_request_count, user_key_tokens, user_key_cost_microdollars)
    SELECT date_trunc(''hour'', s.created_at, ''UTC''), s.user_id, COALESCE(s.provider, ''''),
           COALESCE(s.model, ''''), COALESCE(t.agent_name, ''''), s.synthetic,
           SUM(s.sign), SUM(s.sign * COALESCE(s.tokens_used, 0)),
           SUM(s.sign * COALESCE(s.input_tokens, 0)), SUM(s.sign * COALESCE(s.output_tokens, 0)),
           SUM(s.sign * COALESCE(s.reasoning_tokens, 0)),
           SUM(s.sign * COALESCE(s.cache_read_tokens, 0)),
           SUM(s.sign * COALESCE(s.cache_creation_tokens, 0)), SUM(s.sign * s.cost_microdollars),
           SUM(s.sign * COALESCE(s.latency_ms, 0)),
           COALESCE(SUM(s.sign) FILTER (WHERE s.latency_ms IS NOT NULL), 0),
           COALESCE(SUM(s.sign) FILTER (WHERE s.cache_hit), 0),
           COALESCE(SUM(s.sign) FILTER (WHERE s.billing_source = ''user_key''), 0),
           COALESCE(SUM(s.sign * COALESCE(s.tokens_used, 0))
               FILTER (WHERE s.billing_source = ''user_key''), 0),
           COALESCE(SUM(s.sign * COALESCE(s.cost_microdollars, 0))
               FILTER (WHERE s.billing_source = ''user_key''), 0)
    FROM (%s) s
    LEFT JOIN agent_tasks t ON t.task_id = s.task_id
    GROUP BY 1, 2, 3, 4, 5, 6
    ORDER BY 1, 2, 3, 4, 5, 6
    ON CONFLICT (bucket, user_id, provider, model, agent_name, synthetic) DO UPDATE SET
        request_count = r.request_count + EXCLUDED.request_count,
        tokens = r.tokens + EXCLUDED.tokens,
        input_tokens = r.input_tokens + EXCLUDED.input_tokens,
        output_tokens = r.output_tokens + EXCLUDED.output_tokens,
        reasoning_tokens = r.reasoning_tokens + EXCLUDED.reasoning_tokens,
        cache_read_tokens = r.cache_read_tokens + EXCLUDED.cache_read_tokens,
        cache_creation_tokens = r.cache_creation_tokens + EXCLUDED.cache_creation_tokens,
        cost_microdollars = r.cost_microdollars + EXCLUDED.cost_microdollars,
        latency_sum_ms = r.latency_sum_ms + EXCLUDED.latency_sum_ms,
        latency_n = r.latency_n + EXCLUDED.latency_n,
        cache_hits = r.cache_hits + EXCLUDED.cache_hits,
        user_key_request_count = r.user_key_request_count + EXCLUDED.user_key_request_count,
        user_key_tokens = r.user_key_tokens + EXCLUDED.user_key_tokens,
        user_key_cost_microdollars = r.user_key_cost_microdollars + EXCLUDED.user_key_cost_microdollars
', source)
$$;

CREATE OR REPLACE FUNCTION refresh_ai_usage_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    DELETE FROM ai_usage_rollup_hourly WHERE bucket = ANY(hours);
    EXECUTE ai_usage_rollup_upsert_sql(
        'SELECT a.*, 1 AS sign FROM ai_requests a JOIN unnest($1) h(b) '
        'ON a.created_at >= h.b AND a.created_at < h.b + interval ''1 hour''')
    USING hours;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION ai_usage_rollup_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    changed CONSTANT TEXT := '(n.created_at, n.user_id, n.provider, n.model, n.task_id, n.synthetic, '
        'n.tokens_used, n.input_tokens, n.output_tokens, n.reasoning_tokens, n.cache_read_tokens, '
        'n.cache_creation_tokens, n.cost_microdollars, n.latency_ms, n.cache_hit, '
        'n.billing_source) IS DISTINCT FROM '
        '(o.created_at, o.user_id, o.provider, o.model, o.task_id, o.synthetic, o.tokens_used, '
        'o.input_tokens, o.output_tokens, o.reasoning_tokens, o.cache_read_tokens, '
        'o.cache_creation_tokens, o.cost_microdollars, o.latency_ms, o.cache_hit, '
        'o.billing_source)';
BEGIN
    IF TG_OP = 'INSERT' THEN
        EXECUTE ai_usage_rollup_upsert_sql('SELECT n.*, 1 AS sign FROM new_rows n');
        RETURN NULL;
    ELSIF TG_OP = 'UPDATE' THEN
        EXECUTE ai_usage_rollup_upsert_sql(
            'SELECT n.*, 1 AS sign FROM new_rows n JOIN old_rows o ON o.id = n.id WHERE ' || changed
            || ' UNION ALL SELECT o.*, -1 AS sign FROM old_rows o JOIN new_rows n ON n.id = o.id WHERE '
            || changed);
    ELSE
        EXECUTE ai_usage_rollup_upsert_sql('SELECT o.*, -1 AS sign FROM old_rows o');
    END IF;
    DELETE FROM ai_usage_rollup_hourly
    WHERE request_count = 0
      AND bucket = ANY(ARRAY(SELECT DISTINCT date_trunc('hour', o.created_at, 'UTC') FROM old_rows o));
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS ai_usage_rollup_insert ON ai_requests;
CREATE TRIGGER ai_usage_rollup_insert
    AFTER INSERT ON ai_requests
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_usage_rollup_apply();

DROP TRIGGER IF EXISTS ai_usage_rollup_update ON ai_requests;
CREATE TRIGGER ai_usage_rollup_update
    AFTER UPDATE ON ai_requests
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_usage_rollup_apply();

DROP TRIGGER IF EXISTS ai_usage_rollup_delete ON ai_requests;
CREATE TRIGGER ai_usage_rollup_delete
    AFTER DELETE ON ai_requests
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_usage_rollup_apply();
