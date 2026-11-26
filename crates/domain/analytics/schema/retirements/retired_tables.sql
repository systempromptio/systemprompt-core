-- Tables this member no longer owns. They are declared, not executed: the
-- install releases this member's record of each, and the member that now
-- declares the table takes it over with its rows.
--
-- tool_usage_rollup_hourly: its triggers sit on `mcp_tool_executions`, so
-- the `mcp` member owns it.
-- agent_task_rollup_hourly: its triggers sit on `agent_tasks`, so the
-- `agent` member owns it.
DROP TABLE IF EXISTS tool_usage_rollup_hourly;
DROP TABLE IF EXISTS agent_task_rollup_hourly;
