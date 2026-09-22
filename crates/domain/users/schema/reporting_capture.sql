CREATE OR REPLACE VIEW reporting_source_users AS
SELECT id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT id,name,status,roles,created_at FROM users) fact;
DROP TRIGGER IF EXISTS reporting_capture ON users;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON users
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('users', 'id', 'id,name,status,roles,created_at');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON users
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('users', 'id', 'id,name,status,roles,created_at');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON users
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('users', 'id', 'id,name,status,roles,created_at');

CREATE OR REPLACE VIEW reporting_source_user_sessions AS
SELECT session_id::text AS entity_key, to_jsonb(fact) AS row
FROM (SELECT session_id,user_id,started_at,last_activity_at,ended_at,duration_seconds,user_type,converted_at,expires_at,client_id,client_type,request_count,avg_response_time_ms,success_rate,error_count,task_count,message_count,ai_request_count,total_tokens_used,total_ai_cost_microdollars,ip_address,user_agent,device_type,browser,os,country,region,city,preferred_locale,referrer_source,referrer_url,landing_page,entry_url,utm_source,utm_medium,utm_campaign,utm_content,utm_term,endpoints_accessed,fingerprint_hash,is_bot,is_ai_crawler,is_scanner,is_behavioral_bot,behavioral_bot_reason,behavioral_bot_score,session_source,revoked_at FROM user_sessions) fact;
DROP TRIGGER IF EXISTS reporting_capture ON user_sessions;
CREATE OR REPLACE TRIGGER reporting_capture_insert AFTER INSERT ON user_sessions
REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('user_sessions', 'session_id', 'session_id,user_id,started_at,last_activity_at,ended_at,duration_seconds,user_type,converted_at,expires_at,client_id,client_type,request_count,avg_response_time_ms,success_rate,error_count,task_count,message_count,ai_request_count,total_tokens_used,total_ai_cost_microdollars,ip_address,user_agent,device_type,browser,os,country,region,city,preferred_locale,referrer_source,referrer_url,landing_page,entry_url,utm_source,utm_medium,utm_campaign,utm_content,utm_term,endpoints_accessed,fingerprint_hash,is_bot,is_ai_crawler,is_scanner,is_behavioral_bot,behavioral_bot_reason,behavioral_bot_score,session_source,revoked_at');
CREATE OR REPLACE TRIGGER reporting_capture_update AFTER UPDATE ON user_sessions
REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('user_sessions', 'session_id', 'session_id,user_id,started_at,last_activity_at,ended_at,duration_seconds,user_type,converted_at,expires_at,client_id,client_type,request_count,avg_response_time_ms,success_rate,error_count,task_count,message_count,ai_request_count,total_tokens_used,total_ai_cost_microdollars,ip_address,user_agent,device_type,browser,os,country,region,city,preferred_locale,referrer_source,referrer_url,landing_page,entry_url,utm_source,utm_medium,utm_campaign,utm_content,utm_term,endpoints_accessed,fingerprint_hash,is_bot,is_ai_crawler,is_scanner,is_behavioral_bot,behavioral_bot_reason,behavioral_bot_score,session_source,revoked_at');
CREATE OR REPLACE TRIGGER reporting_capture_delete AFTER DELETE ON user_sessions
REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION sp_capture_reporting_change('user_sessions', 'session_id', 'session_id,user_id,started_at,last_activity_at,ended_at,duration_seconds,user_type,converted_at,expires_at,client_id,client_type,request_count,avg_response_time_ms,success_rate,error_count,task_count,message_count,ai_request_count,total_tokens_used,total_ai_cost_microdollars,ip_address,user_agent,device_type,browser,os,country,region,city,preferred_locale,referrer_source,referrer_url,landing_page,entry_url,utm_source,utm_medium,utm_campaign,utm_content,utm_term,endpoints_accessed,fingerprint_hash,is_bot,is_ai_crawler,is_scanner,is_behavioral_bot,behavioral_bot_reason,behavioral_bot_score,session_source,revoked_at');


