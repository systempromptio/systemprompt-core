-- Record SHA-256 digests of the bodies captured on an AI request: the body the
-- client sent, the body we forwarded upstream (these legitimately differ after
-- model retarget, max_tokens clamp, or user_id strip), and the response body.
-- The digest is computed over the full bytes, so a truncated capture is still
-- provable.

ALTER TABLE ai_request_payloads ADD COLUMN IF NOT EXISTS request_body_sha256 TEXT;
ALTER TABLE ai_request_payloads ADD COLUMN IF NOT EXISTS prepared_body_sha256 TEXT;
ALTER TABLE ai_request_payloads ADD COLUMN IF NOT EXISTS response_body_sha256 TEXT;
