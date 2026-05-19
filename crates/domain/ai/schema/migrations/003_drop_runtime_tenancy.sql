-- Remove runtime data-tenancy. The per-request tenant_id was never populated
-- with a real value; gateway policies are now global and quota is keyed per
-- user. Cloud deployment tenancy is unrelated and unaffected.

DROP INDEX IF EXISTS idx_ai_requests_tenant_id;
DROP INDEX IF EXISTS idx_ai_requests_tenant_created;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS tenant_id;

DROP INDEX IF EXISTS idx_ai_gateway_policies_tenant;
ALTER TABLE ai_gateway_policies
    DROP CONSTRAINT IF EXISTS ai_gateway_policies_tenant_id_name_key;
ALTER TABLE ai_gateway_policies DROP COLUMN IF EXISTS tenant_id;
ALTER TABLE ai_gateway_policies
    ADD CONSTRAINT ai_gateway_policies_name_key UNIQUE (name);

DROP INDEX IF EXISTS idx_ai_quota_buckets_tenant_user;
ALTER TABLE ai_quota_buckets
    DROP CONSTRAINT IF EXISTS ai_quota_buckets_tenant_id_user_id_window_seconds_window_st_key;
ALTER TABLE ai_quota_buckets DROP COLUMN IF EXISTS tenant_id;
ALTER TABLE ai_quota_buckets
    ADD CONSTRAINT ai_quota_buckets_user_id_window_seconds_window_start_key
    UNIQUE (user_id, window_seconds, window_start);
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_user ON ai_quota_buckets(user_id);
