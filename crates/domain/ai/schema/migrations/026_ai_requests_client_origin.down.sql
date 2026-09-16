ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_wire_protocol_check;
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_check;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS wire_protocol;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS client_kind;
