-- Server-issued opaque OAuth `state` tokens bound to a stored `return_to`,
-- closing RFC 6749 §10.12 / OAuth 2.1 §4.1.2.2 (browser may not carry the
-- post-login destination). The raw token is never stored: the row is keyed by
-- HMAC-SHA-256(token, oauth_at_rest_pepper). A row is single-use; `consume`
-- updates `consumed_at` atomically and returns the stored destination.
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
