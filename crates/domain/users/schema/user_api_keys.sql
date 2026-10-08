CREATE TABLE IF NOT EXISTS user_api_keys (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    key_prefix VARCHAR(32) NOT NULL UNIQUE,
    key_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    model_allowlist TEXT[],
    budget_microdollars BIGINT,
    max_requests INTEGER,
    request_window_seconds INTEGER,
    CONSTRAINT user_api_keys_window_required CHECK (
        (budget_microdollars IS NULL AND max_requests IS NULL)
        OR COALESCE(request_window_seconds, 0) > 0
    )
);
CREATE INDEX IF NOT EXISTS idx_user_api_keys_user ON user_api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_user_api_keys_prefix_active
    ON user_api_keys(key_prefix)
    WHERE revoked_at IS NULL;
