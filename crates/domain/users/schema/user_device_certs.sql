CREATE TABLE IF NOT EXISTS user_device_certs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    fingerprint VARCHAR(128) NOT NULL UNIQUE,
    label VARCHAR(100) NOT NULL,
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_user_device_certs_user ON user_device_certs(user_id);
CREATE INDEX IF NOT EXISTS idx_user_device_certs_fingerprint_active
    ON user_device_certs(fingerprint)
    WHERE revoked_at IS NULL;
