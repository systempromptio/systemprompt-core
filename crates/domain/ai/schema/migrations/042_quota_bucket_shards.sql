-- Sharded quota windows: a window's usage may span several rows that differ
-- only by `shard`, so a subject shared by every request (an organization, a
-- project) no longer funnels every admission through one locked row. Existing
-- rows become shard 0, which is also the only shard a per-principal window
-- uses, so their totals are unchanged.
ALTER TABLE ai_quota_buckets ADD COLUMN IF NOT EXISTS shard SMALLINT NOT NULL DEFAULT 0;

ALTER TABLE ai_quota_buckets DROP CONSTRAINT IF EXISTS ai_quota_buckets_subject_key;
ALTER TABLE ai_quota_buckets
    ADD CONSTRAINT ai_quota_buckets_subject_key
    UNIQUE (subject_kind, subject_id, window_seconds, window_start, shard);
