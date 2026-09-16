CREATE OR REPLACE VIEW reporting_source_logs AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,timestamp,level,module,message,user_id,session_id,task_id FROM logs) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON logs
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('logs', 'id', 'id,timestamp,level,module,message,user_id,session_id,task_id');

CREATE OR REPLACE VIEW reporting_source_analytics_events AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp FROM analytics_events) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON analytics_events
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('analytics_events', 'id', 'id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp');


