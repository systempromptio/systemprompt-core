UPDATE mcp_artifacts a
SET server_name = e.server_name
FROM mcp_tool_executions e
WHERE a.mcp_execution_id = e.mcp_execution_id
  AND a.server_name = e.tool_name
  AND a.server_name <> e.server_name;
