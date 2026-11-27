-- @no-transaction
-- @supersedes-checksum: 61dfca5ca6c37959
-- @cost: rows=97723 measured=1.1s triggers=live
-- `ai_requests.client_kind` and `ai_request_client_evidence.attested_host`
-- gain 'harness', the ClientKind of the harness desktop app
-- (schema/ai_requests.sql, schema/ai_request_client_evidence.sql). The file
-- runs outside a transaction so each statement commits on its own: the ACCESS
-- EXCLUSIVE lock of a NOT VALID add is released before VALIDATE scans the
-- table under SHARE UPDATE EXCLUSIVE, and reads and writes continue. The wider
-- check takes the narrower check's name once that one is dropped. Every
-- statement is idempotent, so a run interrupted part-way replays cleanly.
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_widened;
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_client_kind_widened
    CHECK (client_kind IN (
        'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'harness', 'pi', 'other', 'internal', 'unknown')) NOT VALID;
ALTER TABLE ai_requests VALIDATE CONSTRAINT ai_requests_client_kind_widened;
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_client_kind_check;
ALTER TABLE ai_requests
    RENAME CONSTRAINT ai_requests_client_kind_widened TO ai_requests_client_kind_check;

ALTER TABLE ai_request_client_evidence
    DROP CONSTRAINT IF EXISTS ai_request_client_evidence_attested_host_widened;
ALTER TABLE ai_request_client_evidence
    ADD CONSTRAINT ai_request_client_evidence_attested_host_widened
    CHECK (attested_host IS NULL OR attested_host IN (
        'claude-code', 'claude-desktop', 'codex', 'opencode', 'hermes', 'harness', 'pi', 'other', 'internal', 'unknown')) NOT VALID;
ALTER TABLE ai_request_client_evidence VALIDATE CONSTRAINT ai_request_client_evidence_attested_host_widened;
ALTER TABLE ai_request_client_evidence DROP CONSTRAINT IF EXISTS ai_request_client_evidence_attested_host_check;
ALTER TABLE ai_request_client_evidence
    RENAME CONSTRAINT ai_request_client_evidence_attested_host_widened TO ai_request_client_evidence_attested_host_check;
