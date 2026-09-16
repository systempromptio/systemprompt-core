CREATE OR REPLACE VIEW reporting_source_agent_tasks AS
SELECT task_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT task_id,context_id,status,status_timestamp,user_id,session_id,trace_id,agent_name,started_at,completed_at,execution_time_ms,error_message,version,created_at,updated_at FROM agent_tasks) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON agent_tasks
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('agent_tasks', 'task_id', 'task_id,context_id,status,status_timestamp,user_id,session_id,trace_id,agent_name,started_at,completed_at,execution_time_ms,error_message,version,created_at,updated_at');

CREATE OR REPLACE VIEW reporting_source_task_messages AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,task_id,created_at FROM task_messages) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON task_messages
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('task_messages', 'id', 'id,task_id,created_at');

CREATE OR REPLACE VIEW reporting_source_user_contexts AS
SELECT context_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT context_id,user_id,session_id,name,kind,created_at,updated_at FROM user_contexts) fact;
CREATE OR REPLACE TRIGGER reporting_capture
AFTER INSERT OR UPDATE OR DELETE ON user_contexts
FOR EACH ROW EXECUTE FUNCTION sp_capture_reporting_change('user_contexts', 'context_id', 'context_id,user_id,session_id,name,kind,created_at,updated_at');


