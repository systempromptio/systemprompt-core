CREATE OR REPLACE VIEW reporting_source_analytics_events AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp FROM analytics_events) fact;
DROP TRIGGER IF EXISTS reporting_capture ON analytics_events;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON analytics_events
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('analytics_events', 'id', 'id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON analytics_events
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('analytics_events', 'id', 'id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON analytics_events
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('analytics_events', 'id', 'id,user_id,session_id,context_id,gateway_conversation_id,provider_request_id,event_type,event_category,severity,endpoint,error_code,response_time_ms,agent_id,task_id,message,metadata,event_data,timestamp');


