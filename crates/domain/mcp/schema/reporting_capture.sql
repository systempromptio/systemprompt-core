CREATE OR REPLACE VIEW reporting_source_mcp_tool_executions AS
SELECT mcp_execution_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at FROM mcp_tool_executions) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON mcp_tool_executions
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('mcp_tool_executions', 'mcp_execution_id', 'mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at');


