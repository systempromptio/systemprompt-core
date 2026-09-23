-- The two reporting-projection tables only the tombstoned 013 dropped. 015
-- does not name them, so a database that stopped at 011 or 012 kept them after
-- 008-013 were tombstoned. Every statement is a `DROP … IF EXISTS`, so it also
-- runs as a no-op on a fresh install.
DROP TABLE IF EXISTS analytics_report_logs;
DROP TABLE IF EXISTS analytics_report_ai_request_messages;
