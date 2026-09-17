-- A deleted user leaves no anonymous sessions behind. SET NULL manufactured
-- rows nobody owned; the delete already clears sessions explicitly, and every
-- table that points at a session (ai_requests, user_contexts,
-- analytics_events) keeps its own SET NULL, so a request outlives its session
-- but never its user.
ALTER TABLE user_sessions DROP CONSTRAINT IF EXISTS user_sessions_user_id_fkey;
ALTER TABLE user_sessions
    ADD CONSTRAINT user_sessions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
