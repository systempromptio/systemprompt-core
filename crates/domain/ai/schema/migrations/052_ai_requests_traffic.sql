-- @supersedes-checksum: 59aded89aa904ec0
-- @supersedes-checksum: 3557bdf9e955a0c2
-- @supersedes-checksum: ed91b66d199d1272
-- @supersedes-checksum: 10f783be59aa1cc5
-- @cost: rows=300000 measured=130ms triggers=suspended
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
    ADD CONSTRAINT ai_requests_traffic_check CHECK (traffic IN ('production', 'evaluation')) NOT VALID;

ALTER TABLE ai_usage_rollup_hourly
    ADD COLUMN IF NOT EXISTS traffic TEXT NOT NULL DEFAULT 'production';

-- Why: bounded: matches no row. The runner suspends row triggers only on tables a
-- parsed DML statement writes, and the guarded updates below hide theirs in a DO block.
UPDATE ai_requests SET traffic = traffic WHERE FALSE;
UPDATE ai_usage_rollup_hourly SET traffic = traffic WHERE FALSE;

-- Why: bounded: only evaluation rows carry `synthetic`, a small slice of the table.
-- Why: Postgres: a schema laid down before ai 040 never had `synthetic`, so the
-- statement is only valid where the column exists.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_schema = current_schema()
                 AND table_name = 'ai_requests' AND column_name = 'synthetic') THEN
        UPDATE ai_requests SET traffic = 'evaluation' WHERE synthetic;
    END IF;
END $$;
ALTER TABLE ai_requests VALIDATE CONSTRAINT ai_requests_traffic_check;
-- Why: bounded: the hourly rollup holds one row per bucket and key, not per request.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_schema = current_schema()
                 AND table_name = 'ai_usage_rollup_hourly' AND column_name = 'synthetic') THEN
        UPDATE ai_usage_rollup_hourly SET traffic = 'evaluation' WHERE synthetic;
    END IF;
END $$;

ALTER TABLE ai_usage_rollup_hourly ALTER COLUMN traffic DROP DEFAULT;
ALTER TABLE ai_usage_rollup_hourly DROP CONSTRAINT IF EXISTS ai_usage_rollup_hourly_pkey;
ALTER TABLE ai_usage_rollup_hourly DROP CONSTRAINT IF EXISTS ai_usage_rollup_hourly_traffic_check;
ALTER TABLE ai_usage_rollup_hourly DROP COLUMN IF EXISTS synthetic;
ALTER TABLE ai_usage_rollup_hourly
    ADD CONSTRAINT ai_usage_rollup_hourly_traffic_check
        CHECK (traffic IN ('production', 'evaluation')) NOT VALID;
ALTER TABLE ai_usage_rollup_hourly VALIDATE CONSTRAINT ai_usage_rollup_hourly_traffic_check;
ALTER TABLE ai_usage_rollup_hourly
    ADD CONSTRAINT ai_usage_rollup_hourly_pkey
        PRIMARY KEY (bucket, user_id, provider, model, agent_name, traffic);

ALTER TABLE ai_requests DROP COLUMN IF EXISTS synthetic;
