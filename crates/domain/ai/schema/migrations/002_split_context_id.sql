-- Split conflated context_id into typed namespaces.
-- - context_id: UUID v4 user-owned context (kept; cleaned to NULL where it
--   was holding ctx_<hex> or other non-UUID strings).
-- - gateway_conversation_id: deterministic ctx_<16hex> bridge cache key.
-- - provider_request_id: opaque upstream provider trace.

ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS gateway_conversation_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS provider_request_id VARCHAR(255);

UPDATE ai_requests
SET gateway_conversation_id = context_id,
    context_id = NULL
WHERE gateway_conversation_id IS NULL
  AND context_id ~ '^ctx_[0-9a-f]{16}$';

UPDATE ai_requests
SET provider_request_id = context_id,
    context_id = NULL
WHERE provider_request_id IS NULL
  AND context_id IS NOT NULL
  AND context_id !~ '^ctx_[0-9a-f]{16}$'
  AND context_id !~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$';

CREATE INDEX IF NOT EXISTS idx_ai_requests_gateway_conversation_id
    ON ai_requests(gateway_conversation_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_provider_request_id
    ON ai_requests(provider_request_id);
