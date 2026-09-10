-- Separate upstream provider time from total gateway time.
--
-- `latency_ms` measures the whole request as the caller experienced it, so
-- gateway overhead — governance, quota, protocol translation, audit — could not
-- be derived from it. `upstream_latency_ms` brackets the provider call alone;
-- overhead is the difference. It is NULL for a request that never reached a
-- provider, and for every row written before this migration.
ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS upstream_latency_ms INTEGER;
