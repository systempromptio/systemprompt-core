CREATE TABLE IF NOT EXISTS bridge_exchange_codes (
    code_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    ui_client_id VARCHAR(255),
    ui_redirect_uri TEXT,
    ui_scope TEXT,
    ui_code_challenge TEXT,
    ui_authenticated_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_bridge_exchange_codes_user ON bridge_exchange_codes(user_id);
CREATE INDEX IF NOT EXISTS idx_bridge_exchange_codes_active
    ON bridge_exchange_codes(code_hash)
    WHERE consumed_at IS NULL;
