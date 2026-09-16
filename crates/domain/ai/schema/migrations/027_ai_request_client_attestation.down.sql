DROP TABLE IF EXISTS ai_request_client_evidence;
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_attestation_check;
ALTER TABLE ai_requests DROP COLUMN IF EXISTS client_attestation;
UPDATE ai_requests SET client_kind = 'other' WHERE client_kind = 'pi';
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_check;
ALTER TABLE ai_requests ADD CONSTRAINT ai_requests_client_kind_check CHECK (client_kind IN (
    'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'other', 'internal', 'unknown'));
