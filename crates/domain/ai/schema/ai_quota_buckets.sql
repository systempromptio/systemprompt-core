-- Declares the post-011 target shape: fresh installs stamp migrations
-- instead of executing them, so this DDL alone must produce the final
-- schema. Established databases still reach the same shape through
-- migrations 003/011, whose guards tolerate both the legacy user_id
-- layout and this one.
CREATE TABLE IF NOT EXISTS ai_quota_buckets (
    id TEXT PRIMARY KEY,
    subject_id VARCHAR(255) NOT NULL,
    subject_kind TEXT NOT NULL DEFAULT 'user',
    window_seconds INTEGER NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ai_quota_buckets_subject_key
        UNIQUE (subject_kind, subject_id, window_seconds, window_start)
);
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_window ON ai_quota_buckets(window_start);
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_subject
    ON ai_quota_buckets(subject_kind, subject_id);
