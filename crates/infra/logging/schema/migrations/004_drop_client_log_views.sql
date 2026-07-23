-- Unconsumed per-client log views removed; the logs table and its
-- (client_id, timestamp) composite indexes remain the query surface.
DROP VIEW IF EXISTS v_log_analytics_by_client CASCADE;
DROP VIEW IF EXISTS v_client_errors CASCADE;
