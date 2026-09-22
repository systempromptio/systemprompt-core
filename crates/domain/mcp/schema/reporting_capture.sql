-- Column list = the versioned reporting contract for this source
-- (analytics: reporting_privacy.sql, apply_reporting_privacy_row). A capture
-- that emits a column the contract does not list makes every privacy
-- transaction — user delete, anonymous cleanup — raise on delivery.
CREATE OR REPLACE VIEW reporting_source_mcp_tool_executions AS
SELECT mcp_execution_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at FROM mcp_tool_executions) fact;
DROP TRIGGER IF EXISTS reporting_capture ON mcp_tool_executions;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON mcp_tool_executions
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('mcp_tool_executions', 'mcp_execution_id', 'mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON mcp_tool_executions
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('mcp_tool_executions', 'mcp_execution_id', 'mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON mcp_tool_executions
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('mcp_tool_executions', 'mcp_execution_id', 'mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at');
