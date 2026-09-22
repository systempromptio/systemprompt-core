CREATE OR REPLACE VIEW reporting_source_ai_requests AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at,message_count FROM ai_requests) fact;
DROP TRIGGER IF EXISTS reporting_capture ON ai_requests;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON ai_requests
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('ai_requests', 'id', 'id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at,message_count');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON ai_requests
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('ai_requests', 'id', 'id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at,message_count');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON ai_requests
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('ai_requests', 'id', 'id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at,message_count');

-- `message_count` on ai_requests is the projected stand-in for the message
-- table, which is no longer captured: one counter maintained per statement
-- instead of one fact per stored message.
CREATE OR REPLACE FUNCTION sp_ai_request_message_count() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE ai_requests r SET message_count = r.message_count + c.n
        FROM (SELECT request_id, count(*)::integer AS n FROM new_rows GROUP BY request_id) c
        WHERE r.id = c.request_id;
    ELSE
        UPDATE ai_requests r SET message_count = GREATEST(r.message_count - c.n, 0)
        FROM (SELECT request_id, count(*)::integer AS n FROM old_rows GROUP BY request_id) c
        WHERE r.id = c.request_id;
    END IF;
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER message_count_insert AFTER INSERT ON ai_request_messages
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_ai_request_message_count();
CREATE OR REPLACE TRIGGER message_count_delete AFTER DELETE ON ai_request_messages
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_ai_request_message_count();
