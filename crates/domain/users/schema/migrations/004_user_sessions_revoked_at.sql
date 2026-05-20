ALTER TABLE user_sessions
    ADD COLUMN IF NOT EXISTS revoked_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_user_sessions_revoked
    ON user_sessions (revoked_at)
    WHERE revoked_at IS NOT NULL;
