-- Drop single-column log indexes covered by a (col, timestamp DESC) composite:
-- redundant index maintenance on every insert into a write-heavy table.
DROP INDEX IF EXISTS idx_logs_level;
DROP INDEX IF EXISTS idx_logs_user_id;
DROP INDEX IF EXISTS idx_logs_session_id;
DROP INDEX IF EXISTS idx_logs_context_id;
DROP INDEX IF EXISTS idx_logs_client_id;
