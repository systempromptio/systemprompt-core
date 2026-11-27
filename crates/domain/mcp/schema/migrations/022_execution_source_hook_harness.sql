-- @no-transaction
-- @cost: rows=300000 measured=250ms triggers=live
-- `mcp_tool_executions.source` and `mcp_artifacts.source` gain
-- 'hook_harness', the ExecutionSource of a tool result a harness device
-- reports (schema/mcp_tool_executions.sql, schema/mcp_artifacts.sql). The file
-- runs outside a transaction so each statement commits on its own: the ACCESS
-- EXCLUSIVE lock of a NOT VALID add is released before VALIDATE scans the
-- table under SHARE UPDATE EXCLUSIVE, and reads and writes continue. The wider
-- check takes the narrower check's name once that one is dropped. Every
-- statement is idempotent, so a run interrupted part-way replays cleanly.
ALTER TABLE mcp_tool_executions DROP CONSTRAINT IF EXISTS mcp_tool_executions_source_widened;
ALTER TABLE mcp_tool_executions
    ADD CONSTRAINT mcp_tool_executions_source_widened
    CHECK (source IN (
        'in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')) NOT VALID;
ALTER TABLE mcp_tool_executions VALIDATE CONSTRAINT mcp_tool_executions_source_widened;
ALTER TABLE mcp_tool_executions DROP CONSTRAINT IF EXISTS mcp_tool_executions_source_check;
ALTER TABLE mcp_tool_executions
    RENAME CONSTRAINT mcp_tool_executions_source_widened TO mcp_tool_executions_source_check;

ALTER TABLE mcp_artifacts DROP CONSTRAINT IF EXISTS mcp_artifacts_source_widened;
ALTER TABLE mcp_artifacts
    ADD CONSTRAINT mcp_artifacts_source_widened
    CHECK (source IN (
        'in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode', 'hook_harness')) NOT VALID;
ALTER TABLE mcp_artifacts VALIDATE CONSTRAINT mcp_artifacts_source_widened;
ALTER TABLE mcp_artifacts DROP CONSTRAINT IF EXISTS mcp_artifacts_source_check;
ALTER TABLE mcp_artifacts
    RENAME CONSTRAINT mcp_artifacts_source_widened TO mcp_artifacts_source_check;
