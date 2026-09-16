CREATE TABLE IF NOT EXISTS managed_consumer_credentials (
    device_id TEXT PRIMARY KEY REFERENCES user_device_certs(id) ON DELETE CASCADE,
    issuance_operation TEXT,
    credential_digest TEXT NOT NULL UNIQUE CHECK (credential_digest ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);
