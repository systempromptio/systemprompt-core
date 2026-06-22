CREATE TABLE IF NOT EXISTS id_jag_replay (
    jti         TEXT        PRIMARY KEY,
    expires_at  TIMESTAMPTZ NOT NULL,
    seen_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_id_jag_replay_expires_at ON id_jag_replay (expires_at);
