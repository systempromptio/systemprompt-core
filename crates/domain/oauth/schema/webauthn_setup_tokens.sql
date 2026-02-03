CREATE TABLE IF NOT EXISTS webauthn_setup_tokens (
    id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    purpose VARCHAR(50) NOT NULL CHECK(purpose IN ('credential_link', 'recovery')) DEFAULT 'credential_link',
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_token_hash ON webauthn_setup_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_user_id ON webauthn_setup_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_webauthn_setup_tokens_expires_at ON webauthn_setup_tokens(expires_at);
