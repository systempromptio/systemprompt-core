-- Log rollup: log rows counted per hour, level and module, so the log
-- summary (counts by level, top modules, time range, total) reads a few
-- hundred rollup rows instead of grouping the whole log table on every load.
--
--   bucket      the UTC hour the rows were written in
--   log_count   rows in that hour at that level from that module
--   first_at    the earliest and latest timestamp among them
--   last_at
--
-- An insert adds its rows to their buckets in the writer's statement (an
-- additive counter, `log_rollup_hourly_insert`). A delete cannot shrink a
-- bucket's first/last timestamps from the deleted rows alone, so it queues
-- the touched buckets (keyed by epoch seconds) for `refresh_log_rollup_hourly`,
-- which recounts them from the rows still present; the `log_summary_rollup`
-- job drains that queue. Migration 010 queues every hour that predates the
-- table the same way.
CREATE TABLE IF NOT EXISTS log_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    level VARCHAR(50) NOT NULL,
    module VARCHAR(255) NOT NULL,
    log_count BIGINT NOT NULL,
    first_at TIMESTAMPTZ NOT NULL,
    last_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket, level, module)
);

CREATE OR REPLACE FUNCTION refresh_log_rollup_hourly(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE buckets TIMESTAMPTZ[]; removed BIGINT; written BIGINT;
BEGIN
    buckets := ARRAY(SELECT to_timestamp(k::double precision) FROM unnest(keys) AS k);
    DELETE FROM log_rollup_hourly r WHERE r.bucket = ANY(buckets);
    GET DIAGNOSTICS removed = ROW_COUNT;
    INSERT INTO log_rollup_hourly (bucket, level, module, log_count, first_at, last_at)
    SELECT date_trunc('hour', l.timestamp, 'UTC'), l.level, l.module, COUNT(*),
           MIN(l.timestamp), MAX(l.timestamp)
    FROM unnest(buckets) AS b(bucket)
    JOIN logs l ON l.timestamp >= b.bucket AND l.timestamp < b.bucket + interval '1 hour'
    GROUP BY 1, 2, 3;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN removed + written;
END $$;

CREATE OR REPLACE FUNCTION log_rollup_hourly_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO log_rollup_hourly AS r (bucket, level, module, log_count, first_at, last_at)
    SELECT date_trunc('hour', n.timestamp, 'UTC'), n.level, n.module, COUNT(*),
           MIN(n.timestamp), MAX(n.timestamp)
    FROM new_rows n
    GROUP BY 1, 2, 3
    ON CONFLICT (bucket, level, module) DO UPDATE
    SET log_count = r.log_count + EXCLUDED.log_count,
        first_at = LEAST(r.first_at, EXCLUDED.first_at),
        last_at = GREATEST(r.last_at, EXCLUDED.last_at);
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION log_rollup_hourly_deleted()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(SELECT DISTINCT extract(epoch FROM date_trunc('hour', o.timestamp, 'UTC'))::bigint::text
                  FROM old_rows o);
    IF cardinality(keys) > 0 THEN PERFORM queue_projection_backlog('log_rollup_hourly', keys); END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS log_rollup_hourly_insert ON logs;
CREATE TRIGGER log_rollup_hourly_insert
    AFTER INSERT ON logs
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION log_rollup_hourly_inserted();

DROP TRIGGER IF EXISTS log_rollup_hourly_delete ON logs;
CREATE TRIGGER log_rollup_hourly_delete
    AFTER DELETE ON logs
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION log_rollup_hourly_deleted();
