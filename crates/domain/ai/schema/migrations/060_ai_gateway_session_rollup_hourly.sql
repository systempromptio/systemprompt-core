-- Gateway sessions rolled up per hour (schema/ai_gateway_session_rollup_hourly.sql).
-- The table is the base schema's text; the hour of every taskless request
-- that predates it is queued (one key per row, duplicates collapse on
-- conflict) for `refresh_ai_gateway_session_rollup_hourly`, which the
-- `backfill_rollups` job drains. Requests written after this migration are
-- rolled up by the statement triggers, and a queued hour is rebuilt whole
-- from the ledger.
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

INSERT INTO projection_backlog (projection, key)
SELECT 'ai_gateway_session_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM ai_requests
WHERE task_id IS NULL AND session_id IS NOT NULL
ON CONFLICT DO NOTHING;
