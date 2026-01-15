-- Migration: Add session_source column to distinguish session origins
-- Allows separating visitor traffic from API/programmatic access

ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS session_source VARCHAR(50) DEFAULT 'web'
    CHECK (session_source IN ('web', 'api', 'cli', 'tui', 'oauth', 'unknown'));

COMMENT ON COLUMN user_sessions.session_source IS 'Origin of the session: web (browser), api (programmatic), cli (command line), tui (terminal UI), oauth (token endpoint)';

-- Create index for filtering analytics by source
CREATE INDEX IF NOT EXISTS idx_user_sessions_session_source ON user_sessions(session_source);

-- Create composite index for visitor analytics (web sessions only)
CREATE INDEX IF NOT EXISTS idx_user_sessions_visitor_traffic
    ON user_sessions(started_at)
    WHERE session_source = 'web' AND is_bot = false;

-- Backfill existing sessions based on landing_page presence
-- Sessions with landing_page are likely web visitors
UPDATE user_sessions
SET session_source = 'web'
WHERE landing_page IS NOT NULL AND session_source IS NULL;

-- Sessions without landing_page and from OAuth client are likely API
UPDATE user_sessions
SET session_source = 'api'
WHERE landing_page IS NULL AND session_source IS NULL;
