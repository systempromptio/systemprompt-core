-- The tool-usage-by-agent report reads the hourly tool rollup through
-- `report_agent_tool_usage_hourly` (schema/report_views.sql); the per-execution
-- view it read before is no longer declared.
DROP VIEW IF EXISTS report_agent_tool_executions;
