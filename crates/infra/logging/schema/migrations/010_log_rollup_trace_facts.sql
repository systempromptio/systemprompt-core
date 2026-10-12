-- Log rollup (schema/log_rollup_hourly.sql): the table is the base schema's
-- text; every UTC hour that already holds log rows is queued for
-- `refresh_log_rollup_hourly` (keyed by epoch seconds, one key per hour
-- however many rows share it), and every logged trace for
-- `refresh_trace_facts` (database schema trace_facts.sql). The
-- `log_summary_rollup` and `trace_facts_rollup` jobs drain them.
CREATE TABLE IF NOT EXISTS log_rollup_hourly (
    bucket TIMESTAMPTZ NOT NULL,
    level VARCHAR(50) NOT NULL,
    module VARCHAR(255) NOT NULL,
    log_count BIGINT NOT NULL,
    first_at TIMESTAMPTZ NOT NULL,
    last_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket, level, module)
);

INSERT INTO projection_backlog (projection, key)
SELECT 'log_rollup_hourly', extract(epoch FROM date_trunc('hour', timestamp, 'UTC'))::bigint::text FROM logs
ON CONFLICT DO NOTHING;

INSERT INTO projection_backlog (projection, key)
SELECT 'trace_facts', trace_id FROM logs WHERE trace_id IS NOT NULL AND trace_id <> ''
ON CONFLICT DO NOTHING;
