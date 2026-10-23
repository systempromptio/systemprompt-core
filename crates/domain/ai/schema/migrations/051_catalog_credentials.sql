-- Why: 0.73 keys personal keys and request audit by catalog credential and
-- endpoint (services/ai/credentials.yaml, providers.yaml). Lock footprint on a
-- 100M-row ai_requests: the two ADD COLUMNs are nullable with no default, so
-- each is a catalog-only change taking ACCESS EXCLUSIVE for milliseconds and
-- rewriting no rows; nothing here scans ai_requests. The rename on
-- user_provider_credentials (a small table) is catalog-only as well.
-- ai_catalog_legacy_map is new and empty. Existing rows are moved by
-- `systemprompt admin config migrate --data`, which writes this map from the
-- install's legacy_map.yaml, rewrites user_provider_credentials.credential
-- through it, and backfills ai_requests.credential/endpoint in bounded batches
-- selected through idx_ai_requests_provider_status; never in this file.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_provider_credentials' AND column_name = 'provider'
    ) THEN
        ALTER TABLE user_provider_credentials RENAME COLUMN provider TO credential;
    END IF;
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'user_provider_credentials_user_provider_key'
    ) THEN
        ALTER TABLE user_provider_credentials
            RENAME CONSTRAINT user_provider_credentials_user_provider_key
            TO user_provider_credentials_user_credential_key;
    END IF;
END $$;

ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS credential TEXT;
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS endpoint TEXT;

CREATE TABLE IF NOT EXISTS ai_catalog_legacy_map (
    legacy_provider TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    credential TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
