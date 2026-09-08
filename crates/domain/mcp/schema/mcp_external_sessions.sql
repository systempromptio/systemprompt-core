CREATE TABLE IF NOT EXISTS mcp_external_sessions (
    server_name TEXT NOT NULL,
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    credential_hash BYTEA NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL '1 hour'),
    PRIMARY KEY (server_name, session_id)
);

CREATE INDEX IF NOT EXISTS idx_mcp_external_sessions_expires_at ON mcp_external_sessions(expires_at);
