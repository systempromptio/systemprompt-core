CREATE TABLE IF NOT EXISTS cowork_exchange_codes (
    code_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_cowork_exchange_codes_user ON cowork_exchange_codes(user_id);
CREATE INDEX IF NOT EXISTS idx_cowork_exchange_codes_active
    ON cowork_exchange_codes(code_hash)
    WHERE consumed_at IS NULL;
