CREATE TABLE IF NOT EXISTS bridge_sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tenant_id TEXT,
    bridge_version TEXT NOT NULL,
    os TEXT NOT NULL,
    hostname TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMPTZ,
    forwarded_total BIGINT NOT NULL DEFAULT 0,
    tokens_in_total BIGINT NOT NULL DEFAULT 0,
    tokens_out_total BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_bridge_sessions_user_active
    ON bridge_sessions(user_id, last_heartbeat_at DESC);
CREATE INDEX IF NOT EXISTS idx_bridge_sessions_active
    ON bridge_sessions(last_heartbeat_at DESC);
