-- AI usage rolled up per hour (schema/ai_usage_rollup_hourly.sql). The table
-- is the base schema's text; the hour of every request that predates it is
-- queued (one key per row, duplicates collapse on conflict),
-- for `refresh_ai_usage_rollup_hourly`, which the `backfill_rollups` job
-- drains. Requests written after this migration are rolled up by the
-- statement triggers, and a queued hour is rebuilt whole from the ledger.
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
    PRIMARY KEY (bucket, user_id, provider, model, agent_name, synthetic)
);
CREATE INDEX IF NOT EXISTS idx_ai_usage_rollup_hourly_user_bucket
    ON ai_usage_rollup_hourly(user_id, bucket);

INSERT INTO projection_backlog (projection, key)
SELECT 'ai_usage_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM ai_requests
ON CONFLICT DO NOTHING;
