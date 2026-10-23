-- @compat: breaking
-- Gateway traffic carries a canonical class (schema/ai_requests.sql): the
-- `traffic` column replaces the boolean `synthetic`, and the hourly rollup
-- (schema/ai_usage_rollup_hourly.sql) keys on it instead. A `synthetic` row
-- maps to 'evaluation'. The report views over both tables are dropped here and
-- laid down again by the dependent phase, which also replaces the rollup
-- functions and triggers with their `traffic` form.
DROP VIEW IF EXISTS report_ai_requests;
DROP VIEW IF EXISTS report_ai_usage_hourly;

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS traffic TEXT NOT NULL DEFAULT 'production';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_traffic_check;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_traffic_check CHECK (traffic IN ('production', 'evaluation'));

ALTER TABLE ai_usage_rollup_hourly
    ADD COLUMN IF NOT EXISTS traffic TEXT NOT NULL DEFAULT 'production';

UPDATE ai_requests SET traffic = 'evaluation' WHERE synthetic;
UPDATE ai_usage_rollup_hourly SET traffic = 'evaluation' WHERE synthetic;

ALTER TABLE ai_usage_rollup_hourly ALTER COLUMN traffic DROP DEFAULT;
ALTER TABLE ai_usage_rollup_hourly DROP CONSTRAINT IF EXISTS ai_usage_rollup_hourly_pkey;
ALTER TABLE ai_usage_rollup_hourly DROP CONSTRAINT IF EXISTS ai_usage_rollup_hourly_traffic_check;
ALTER TABLE ai_usage_rollup_hourly DROP COLUMN IF EXISTS synthetic;
ALTER TABLE ai_usage_rollup_hourly
    ADD CONSTRAINT ai_usage_rollup_hourly_traffic_check
        CHECK (traffic IN ('production', 'evaluation')),
    ADD CONSTRAINT ai_usage_rollup_hourly_pkey
        PRIMARY KEY (bucket, user_id, provider, model, agent_name, traffic);

ALTER TABLE ai_requests DROP COLUMN IF EXISTS synthetic;
