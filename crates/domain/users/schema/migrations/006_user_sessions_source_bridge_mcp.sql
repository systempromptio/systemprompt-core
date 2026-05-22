ALTER TABLE user_sessions
    DROP CONSTRAINT IF EXISTS user_sessions_session_source_check;

ALTER TABLE user_sessions
    ADD CONSTRAINT user_sessions_session_source_check
    CHECK (session_source IN ('web', 'api', 'cli', 'oauth', 'mcp', 'bridge', 'unknown'));
