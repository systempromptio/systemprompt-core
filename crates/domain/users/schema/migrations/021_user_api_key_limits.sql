-- Per-key limits and scope bindings.
--
-- A key may be restricted to a model allowlist and given its own spend
-- budget and request ceiling over a window; both ceilings need the window.
-- user_api_key_scopes binds a key to one value per tenant scope dimension,
-- which attributes the key's requests when they carry no
-- x-systemprompt-scope-<dimension> header.
ALTER TABLE user_api_keys
    ADD COLUMN IF NOT EXISTS model_allowlist TEXT[],
    ADD COLUMN IF NOT EXISTS budget_microdollars BIGINT,
    ADD COLUMN IF NOT EXISTS max_requests INTEGER,
    ADD COLUMN IF NOT EXISTS request_window_seconds INTEGER;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'user_api_keys_window_required'
    ) THEN
        ALTER TABLE user_api_keys ADD CONSTRAINT user_api_keys_window_required
            CHECK (
                (budget_microdollars IS NULL AND max_requests IS NULL)
                OR COALESCE(request_window_seconds, 0) > 0
            );
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS user_api_key_scopes (
    key_id TEXT NOT NULL REFERENCES user_api_keys(id) ON DELETE CASCADE,
    dimension TEXT NOT NULL CHECK (length(dimension) BETWEEN 1 AND 64),
    value TEXT NOT NULL CHECK (length(value) > 0),
    PRIMARY KEY (key_id, dimension)
);
