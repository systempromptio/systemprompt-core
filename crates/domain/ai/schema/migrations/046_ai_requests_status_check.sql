-- @cost: rows=300000 measured=66ms triggers=live
-- ai_requests.status is the RequestStatus vocabulary; its four writers only
-- ever wrote these values, so no rewrite precedes the constraint. NOT VALID
-- adds it under a brief lock without scanning; VALIDATE then scans the table
-- under SHARE UPDATE EXCLUSIVE, so reads and writes continue while it runs.
ALTER TABLE ai_requests
    ADD CONSTRAINT ai_requests_status_check
    CHECK (status IN ('pending', 'completed', 'failed', 'rejected')) NOT VALID;
ALTER TABLE ai_requests VALIDATE CONSTRAINT ai_requests_status_check;
