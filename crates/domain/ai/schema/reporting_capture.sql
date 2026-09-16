CREATE OR REPLACE VIEW reporting_source_ai_requests AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at FROM ai_requests) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON ai_requests
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('ai_requests', 'id', 'id,request_id,user_id,session_id,task_id,context_id,gateway_conversation_id,client_session_id,provider_request_id,trace_id,mcp_execution_id,provider,model,requested_model,route_match,temperature,top_p,max_tokens,tokens_used,input_tokens,output_tokens,cost_microdollars,latency_ms,upstream_latency_ms,cache_hit,cache_read_tokens,cache_creation_tokens,reasoning_tokens,is_streaming,status,error_message,actor_kind,actor_id,synthetic,request_kind,instance_id,created_at,updated_at,completed_at');

CREATE OR REPLACE VIEW reporting_source_ai_request_messages AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,request_id,created_at FROM ai_request_messages) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON ai_request_messages
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('ai_request_messages', 'id', 'id,request_id,created_at');


