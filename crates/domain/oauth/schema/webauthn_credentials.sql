CREATE TABLE IF NOT EXISTS webauthn_credentials (
    id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    credential_id BYTEA NOT NULL UNIQUE,
    public_key BYTEA NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    display_name VARCHAR(255) NOT NULL,
    device_type TEXT NOT NULL CHECK(device_type IN ('platform', 'cross-platform')) DEFAULT 'platform',
    transports TEXT NOT NULL DEFAULT '["internal"]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user_id ON webauthn_credentials(user_id);
CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_credential_id ON webauthn_credentials(credential_id);
CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_last_used ON webauthn_credentials(last_used_at);
