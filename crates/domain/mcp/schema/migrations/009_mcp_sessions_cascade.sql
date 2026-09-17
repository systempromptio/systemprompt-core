-- An MCP session belongs to its user; when the user goes the session goes,
-- rather than surviving as a row with no owner. See users migration 016.
ALTER TABLE mcp_sessions DROP CONSTRAINT IF EXISTS mcp_sessions_user_id_fkey;
ALTER TABLE mcp_sessions
    ADD CONSTRAINT mcp_sessions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
