CREATE TABLE IF NOT EXISTS sso_state_bindings (
    state_hash    TEXT PRIMARY KEY,
    provider_id   TEXT        NOT NULL,
    return_to     TEXT        NOT NULL,
    binder_hash   TEXT        NOT NULL,
    request_id    TEXT,
    nonce         TEXT,
    pkce_verifier TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL,
    consumed_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS sso_state_bindings_expires_at_idx ON sso_state_bindings (expires_at);
