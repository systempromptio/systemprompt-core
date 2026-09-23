-- The read contract of the `analytics` reports: one view per source table,
-- so analytics queries only relations it declares while the rows stay where
-- their owners write them. Nothing is copied — every report is current as of
-- its query. Privacy is enforced here, once: rows belonging to a user whose
-- status is `deleted` are invisible to every report; age limits come from
-- retention deleting source rows.
--
-- Dropped and recreated rather than replaced: `SELECT *` fixes the column list
-- at creation, and CREATE OR REPLACE cannot drop or reorder a column a source
-- table lost. Views are stateless, so dropping loses nothing.

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

CREATE VIEW report_users AS
SELECT src.* FROM users src
WHERE src.status <> 'deleted';

CREATE VIEW report_user_sessions AS
SELECT src.* FROM user_sessions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_clean_traffic AS
SELECT src.* FROM v_clean_traffic src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_engaged_traffic AS
SELECT src.* FROM v_engaged_traffic src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_bot_sessions AS
SELECT src.* FROM v_bot_sessions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_ai_requests AS
SELECT src.* FROM ai_requests src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_agent_tasks AS
SELECT src.* FROM agent_tasks src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_task_messages AS
SELECT src.* FROM task_messages src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_user_contexts AS
SELECT src.* FROM user_contexts src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_mcp_tool_executions AS
SELECT src.* FROM mcp_tool_executions src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);

CREATE VIEW report_markdown_content AS
SELECT src.* FROM markdown_content src;

CREATE VIEW report_analytics_events AS
SELECT src.* FROM analytics_events src
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.user_id AND d.status = 'deleted'
);
