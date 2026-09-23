-- The reporting projection is retired: its capture triggers and source views
-- on this extension's tables go, with the privacy routine that locked them.
DROP TRIGGER IF EXISTS reporting_capture_insert ON mcp_tool_executions;
DROP TRIGGER IF EXISTS reporting_capture_update ON mcp_tool_executions;
DROP TRIGGER IF EXISTS reporting_capture_delete ON mcp_tool_executions;
DROP TRIGGER IF EXISTS reporting_capture ON mcp_tool_executions;
DROP VIEW IF EXISTS reporting_source_mcp_tool_executions;
DROP FUNCTION IF EXISTS public.lock_mcp_reporting_sources();
