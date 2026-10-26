-- @cost: rows=300000 measured=6s triggers=live
-- AI requests rolled up per hour and conversation
-- (schema/ai_context_rollup_hourly.sql). The table is the base schema's text;
-- the hour of every request that predates it is queued (one key per row,
-- duplicates collapse on conflict) for `refresh_ai_context_rollup_hourly`,
-- which the `backfill_rollups` job drains. Requests written after this
-- migration are rolled up by the statement triggers, and a queued hour is
-- rebuilt whole from the ledger.
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

INSERT INTO projection_backlog (projection, key)
SELECT 'ai_context_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM ai_requests
ON CONFLICT DO NOTHING;
