ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS client_session_id TEXT;
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS request_kind TEXT NOT NULL DEFAULT 'turn';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_request_kind_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_request_kind_check
    CHECK (request_kind IN ('turn', 'probe', 'utility'));
UPDATE ai_requests SET request_kind = 'probe' WHERE max_tokens IS NOT NULL AND max_tokens <= 1;
CREATE INDEX IF NOT EXISTS idx_ai_requests_client_session_kind ON ai_requests(client_session_id, request_kind);
