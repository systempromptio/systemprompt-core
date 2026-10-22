-- PLACEHOLDER: C-WP6f replaces this file
-- Why: personal keys are stored per catalog credential id, not per provider
-- entry (schema/user_provider_credentials.sql is the base schema's text).
-- Renaming the column and its unique constraint is a catalog-only change: it
-- takes ACCESS EXCLUSIVE on user_provider_credentials (a small table, never
-- ai_requests) for milliseconds and rewrites no rows. Guarded so a fresh
-- install, whose base schema already has `credential`, is a no-op. The real
-- 048 also maps each stored provider name to its credential id.
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
