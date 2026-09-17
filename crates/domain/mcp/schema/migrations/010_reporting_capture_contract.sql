-- The narrow-waist migration (008) widened this source's capture with
-- `source`, `correlation` and `payload_sha256`, columns the reporting
-- contract (analytics reporting_privacy.sql) does not list. Every fact it
-- emitted fails `apply_reporting_privacy_row`, and because delivery runs
-- inside `begin_user_privacy()`, every user delete and anonymous cleanup on
-- an instance that has executed one MCP tool has raised "Reporting row
-- violates versioned contract" since. Restore the contract's column list
-- and strip the three keys from the facts already queued, so they deliver.
CREATE OR REPLACE VIEW reporting_source_mcp_tool_executions AS
SELECT mcp_execution_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at FROM mcp_tool_executions) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON mcp_tool_executions
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('mcp_tool_executions', 'mcp_execution_id', 'mcp_execution_id,tool_name,server_name,started_at,completed_at,execution_time_ms,status,error_message,user_id,session_id,context_id,task_id,trace_id,request_method,request_source,actor_kind,actor_id,ai_tool_call_id,created_at');
UPDATE event_outbox
SET fact = jsonb_set(fact, '{data,row}', (fact->'data'->'row') - 'source' - 'correlation' - 'payload_sha256')
WHERE channel = 'reporting'
  AND processed_at IS NULL
  AND fact->'data'->>'source' = 'mcp_tool_executions'
  AND jsonb_typeof(fact->'data'->'row') = 'object';
