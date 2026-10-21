-- A provider key one user stored for themselves (a personal subscription such
-- as OpenCode Go). `ciphertext` is the key sealed with ChaCha20-Poly1305 under
-- `encryption_master_key` (systemprompt_security::at_rest), hex-encoded with
-- its 12-byte nonce prepended; `nonce` repeats that nonce for operators, it is
-- never read back. One key per user and provider: a subscription is personal.
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
