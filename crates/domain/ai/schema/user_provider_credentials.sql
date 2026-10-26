-- A key one member stored for themselves under a personal catalog credential
-- (`services/ai/credentials.yaml`, `kind: personal`; e.g. OpenCode Go). `ciphertext` is the key sealed with ChaCha20-Poly1305 under
-- `encryption_master_key` (systemprompt_security::at_rest), hex-encoded with
-- its 12-byte nonce prepended; `nonce` repeats that nonce for operators, it is
-- never read back. One key per user and credential: a subscription is personal,
-- and one stored key serves every endpoint the credential authenticates.
CREATE TABLE IF NOT EXISTS user_provider_credentials (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential TEXT NOT NULL,
    ciphertext TEXT NOT NULL,
    nonce TEXT NOT NULL,
    label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    CONSTRAINT user_provider_credentials_user_credential_key UNIQUE (user_id, credential)
);
