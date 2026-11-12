-- The hourly AI usage rollup carries the share of each row billed to the
-- login the client presented (schema/ai_usage_rollup_hourly.sql). The columns
-- are the base schema's text. No hour is queued for a rebuild: no request
-- carries `billing_source = 'user_subscription'` before this migration.
ALTER TABLE ai_usage_rollup_hourly
    ADD COLUMN IF NOT EXISTS user_subscription_request_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS user_subscription_tokens BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS user_subscription_cost_microdollars BIGINT NOT NULL DEFAULT 0;
