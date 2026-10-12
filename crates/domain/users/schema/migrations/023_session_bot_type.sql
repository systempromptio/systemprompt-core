-- Session bot type (schema/session_bot_type.sql): the taxonomy label is
-- stored on the row instead of re-derived by `v_bot_sessions` on every read.
-- The column is the base schema's text; every bot session that predates it is
-- queued for `refresh_session_bot_type`, which the `session_bot_type_rollup`
-- job drains. Sessions written from here on are stamped by the statement
-- triggers. Keys only; nothing is rebuilt here.
ALTER TABLE user_sessions ADD COLUMN IF NOT EXISTS bot_type TEXT;

INSERT INTO projection_backlog (projection, key)
SELECT 'session_bot_type', session_id FROM user_sessions
WHERE bot_type IS NULL AND (is_bot OR is_ai_crawler OR is_scanner OR is_behavioral_bot)
ON CONFLICT DO NOTHING;
