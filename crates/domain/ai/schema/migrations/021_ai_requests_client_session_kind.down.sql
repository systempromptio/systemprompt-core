DROP INDEX IF EXISTS idx_ai_requests_client_session_kind;
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_request_kind_check;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS request_kind;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS client_session_id;
