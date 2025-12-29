-- Migration: Rename mcp_server_name to server_name in mcp_tool_executions table

-- Step 1: Rename the column
ALTER TABLE mcp_tool_executions
    RENAME COLUMN mcp_server_name TO server_name;

-- Step 2: Drop old indexes that reference mcp_server_name
DROP INDEX IF EXISTS idx_mcp_tool_executions_server_name;
DROP INDEX IF EXISTS idx_mcp_tool_executions_server_tool;
DROP INDEX IF EXISTS idx_mcp_tool_executions_server_status;

-- Step 3: Recreate indexes with the new column name
CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_server_name ON mcp_tool_executions(server_name);
CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_server_tool ON mcp_tool_executions(server_name, tool_name);
CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_server_status ON mcp_tool_executions(server_name, status);
