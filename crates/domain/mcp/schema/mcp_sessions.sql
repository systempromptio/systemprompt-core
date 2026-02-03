CREATE TABLE IF NOT EXISTS mcp_sessions (
    session_id TEXT PRIMARY KEY,
    user_id VARCHAR(255) REFERENCES users(id) ON DELETE SET NULL,
    mcp_server_id TEXT,
    last_event_id TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'closed', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL '24 hours')
);

CREATE INDEX IF NOT EXISTS idx_mcp_sessions_user_id ON mcp_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_mcp_sessions_status ON mcp_sessions(status);
CREATE INDEX IF NOT EXISTS idx_mcp_sessions_expires_at ON mcp_sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_mcp_sessions_last_activity ON mcp_sessions(last_activity_at);
CREATE INDEX IF NOT EXISTS idx_mcp_sessions_active ON mcp_sessions(status) WHERE status = 'active';
