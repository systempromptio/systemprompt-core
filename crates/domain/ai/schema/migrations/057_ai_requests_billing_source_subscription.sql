-- `ai_requests.billing_source` gains 'user_subscription': a request sent with
-- the credential the client presented, billed to that member's own vendor
-- subscription (schema/ai_requests.sql). The wider check is added NOT VALID
-- under a brief lock without scanning, VALIDATE then scans under SHARE UPDATE
-- EXCLUSIVE so reads and writes continue, and it takes the narrower check's
-- name once that one is dropped.
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_billing_source_widened
    CHECK (billing_source IN ('platform', 'user_key', 'user_subscription')) NOT VALID;
ALTER TABLE ai_requests VALIDATE CONSTRAINT ai_requests_billing_source_widened;
ALTER TABLE ai_requests DROP CONSTRAINT IF EXISTS ai_requests_billing_source_check;
ALTER TABLE ai_requests
    RENAME CONSTRAINT ai_requests_billing_source_widened TO ai_requests_billing_source_check;
