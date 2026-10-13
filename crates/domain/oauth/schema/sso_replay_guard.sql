CREATE TABLE IF NOT EXISTS sso_replay_guard (
    scope      TEXT        NOT NULL,
    id         TEXT        NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (scope, id)
);
CREATE INDEX IF NOT EXISTS sso_replay_guard_expires_at_idx ON sso_replay_guard (expires_at);
