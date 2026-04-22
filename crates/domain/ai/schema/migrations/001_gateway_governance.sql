ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS tenant_id VARCHAR(255);
CREATE INDEX IF NOT EXISTS idx_ai_requests_tenant_id ON ai_requests(tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_tenant_created ON ai_requests(tenant_id, created_at);

ALTER TABLE ai_request_tool_calls ADD COLUMN IF NOT EXISTS tool_result_payload JSONB;
