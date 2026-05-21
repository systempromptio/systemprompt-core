CREATE TABLE IF NOT EXISTS oauth_state_bindings (
    state_token_hash TEXT PRIMARY KEY,
    return_to        TEXT        NOT NULL,
    client_id        TEXT        NOT NULL,
    redirect_uri     TEXT        NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at       TIMESTAMPTZ NOT NULL,
    consumed_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS oauth_state_bindings_expires_at_idx ON oauth_state_bindings (expires_at);
