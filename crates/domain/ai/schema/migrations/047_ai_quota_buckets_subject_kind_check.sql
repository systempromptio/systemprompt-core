-- @cost: rows=300000 measured=30ms triggers=live
-- ai_quota_buckets.subject_kind is an open vocabulary: the two principals
-- (user, api_key) plus any scope dimension a gateway policy keys a window by,
-- so the constraint pins only that a kind is present. NOT VALID adds it
-- under a brief lock without scanning; VALIDATE then scans under SHARE
-- UPDATE EXCLUSIVE, so reads and writes continue while it runs.
ALTER TABLE ai_quota_buckets
    ADD CONSTRAINT ai_quota_buckets_subject_kind_check
    CHECK (length(subject_kind) > 0) NOT VALID;
ALTER TABLE ai_quota_buckets VALIDATE CONSTRAINT ai_quota_buckets_subject_kind_check;
