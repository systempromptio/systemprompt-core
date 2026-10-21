-- The hourly AI usage rollup carries the personal-key share of each row
-- (schema/ai_usage_rollup_hourly.sql). The columns are the base schema's
-- text; every hour holding a request billed to a personal key is queued for
-- `refresh_ai_usage_rollup_hourly`, which the `backfill_rollups` job drains.
ALTER TABLE ai_usage_rollup_hourly
    ADD COLUMN IF NOT EXISTS user_key_request_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS user_key_tokens BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS user_key_cost_microdollars BIGINT NOT NULL DEFAULT 0;

INSERT INTO projection_backlog (projection, key)
SELECT DISTINCT 'ai_usage_rollup_hourly',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM ai_requests
WHERE billing_source = 'user_key'
ON CONFLICT DO NOTHING;
