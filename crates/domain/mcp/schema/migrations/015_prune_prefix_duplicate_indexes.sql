-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another index on the same table. `mcp_tool_executions` carried 26
-- indexes for 1 MB of heap; a prefix index answers no query its superset
-- cannot and costs a write on every insert.
--
-- Each one names the index that already covers it. None is UNIQUE and none
-- backs a constraint.

-- covered by mcp_artifacts_artifact_id_key (same column)
DROP INDEX IF EXISTS idx_mcp_artifacts_artifact_id;
-- covered by idx_mcp_artifacts_type_created
DROP INDEX IF EXISTS idx_mcp_artifacts_artifact_type;
-- covered by idx_mcp_artifacts_server_created
DROP INDEX IF EXISTS idx_mcp_artifacts_server_name;

-- covered by mcp_tool_executions_pkey (same column)
DROP INDEX IF EXISTS idx_mcp_tool_executions_mcp_execution_id;
-- covered by idx_mcp_tool_executions_context_created
DROP INDEX IF EXISTS idx_mcp_tool_executions_context_id;
-- covered by idx_mcp_tool_executions_server_tool and ..._server_status
DROP INDEX IF EXISTS idx_mcp_tool_executions_server_name;
-- covered by idx_mcp_tool_executions_session_tool
DROP INDEX IF EXISTS idx_mcp_tool_executions_session_id;
-- covered by idx_mcp_tool_executions_tool_status and ..._tool_started
DROP INDEX IF EXISTS idx_mcp_tool_executions_tool_name;
-- covered by mcp_tool_executions_owner_id and idx_mcp_tool_executions_user_created
DROP INDEX IF EXISTS idx_mcp_tool_executions_user_id;
