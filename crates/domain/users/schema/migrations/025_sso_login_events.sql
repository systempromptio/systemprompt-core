-- One row per browser-redirect SSO sign-in attempt, success or refusal, so a
-- refused login is as visible as an accepted one. `user_id` is known only on
-- success; `reason` only on refusal. Same DDL as schema/sso_login_events.sql.
CREATE TABLE IF NOT EXISTS sso_login_events (
    id           BIGSERIAL PRIMARY KEY,
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    provider_id  TEXT        NOT NULL,
    issuer       TEXT        NOT NULL,
    outcome      TEXT        NOT NULL,
    reason       TEXT,
    user_id      TEXT REFERENCES users(id) ON DELETE SET NULL,
    email        TEXT,
    external_sub TEXT,
    ip           INET,
    groups_count INT
);
CREATE INDEX IF NOT EXISTS idx_sso_login_events_occurred_at ON sso_login_events (occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_sso_login_events_user ON sso_login_events (user_id);
CREATE INDEX IF NOT EXISTS idx_sso_login_events_provider ON sso_login_events (provider_id, occurred_at DESC);
