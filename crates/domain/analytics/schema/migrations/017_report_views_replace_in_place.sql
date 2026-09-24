-- The report views are replaced in place from this release on instead of
-- being dropped on every boot. An established database may still hold a
-- definition CREATE OR REPLACE cannot replace (a preview build defined some of
-- them with `SELECT *`, whose column order differs), so they are dropped once
-- here and the dependent phase lays every one down again. Dependents first.
DROP VIEW IF EXISTS report_analytics_events;
DROP VIEW IF EXISTS report_markdown_content;
DROP VIEW IF EXISTS report_mcp_tool_executions;
DROP VIEW IF EXISTS report_user_contexts;
DROP VIEW IF EXISTS report_task_messages;
DROP VIEW IF EXISTS report_agent_tasks;
DROP VIEW IF EXISTS report_ai_requests;
DROP VIEW IF EXISTS report_bot_sessions;
DROP VIEW IF EXISTS report_engaged_traffic;
DROP VIEW IF EXISTS report_clean_traffic;
DROP VIEW IF EXISTS report_user_sessions;
DROP VIEW IF EXISTS report_users;
