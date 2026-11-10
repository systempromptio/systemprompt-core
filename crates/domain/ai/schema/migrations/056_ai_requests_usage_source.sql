-- @cost: rows=300000 measured=40ms triggers=suspended
-- Record where a completed request's usage came from (schema/ai_requests.sql):
-- 'reported' when the provider sent a usage frame, 'reservation' when a stream
-- completed without one and quota settled to the admission reservation. The
-- request stays `completed`; this is an accounting annotation. The constant
-- default makes the add a catalog change, so no row is rewritten, and a binary
-- that never writes the column keeps working.
ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS usage_source TEXT NOT NULL DEFAULT 'reported';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_usage_source_check;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_usage_source_check
        CHECK (usage_source IN ('reported', 'reservation')) NOT VALID;
ALTER TABLE ai_requests VALIDATE CONSTRAINT ai_requests_usage_source_check;
