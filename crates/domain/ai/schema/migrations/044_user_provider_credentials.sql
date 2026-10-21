-- Personal subscription keys: the per-user credential store
-- (schema/user_provider_credentials.sql, the base schema's text) and the
-- `billing_source` column that marks a request billed to one of them, so its
-- estimated cost is recorded but never settles platform spend.
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS billing_source TEXT NOT NULL DEFAULT 'platform'
    CONSTRAINT ai_requests_billing_source_check CHECK (billing_source IN ('platform', 'user_key'));

CREATE TABLE IF NOT EXISTS user_provider_credentials (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    ciphertext TEXT NOT NULL,
    nonce TEXT NOT NULL,
    label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    CONSTRAINT user_provider_credentials_user_provider_key UNIQUE (user_id, provider)
);
