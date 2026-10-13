-- Browser-redirect SSO correlation and replay stores.
--
-- `sso_state_bindings` remembers what an SSO `start` needs at `finish`: the
-- provider, the post-login destination, the protocol nonces, and a hash of the
-- binder cookie that ties the return to the browser that started it. The raw
-- state and binder are never stored: both are keyed by
-- HMAC-SHA-256(value, oauth_at_rest_pepper). A row is single-use.
--
-- `sso_replay_guard` records each assertion id the first time it is accepted,
-- scoped per provider, until the assertion itself expires.
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

CREATE TABLE IF NOT EXISTS sso_replay_guard (
    scope      TEXT        NOT NULL,
    id         TEXT        NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (scope, id)
);
CREATE INDEX IF NOT EXISTS sso_replay_guard_expires_at_idx ON sso_replay_guard (expires_at);
