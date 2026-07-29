-- The CREATE TABLE deliberately declares the pre-011 shape (user_id, and the
-- inline UNIQUE whose auto-generated name migration 003 guards on): the
-- installer runs structural DDL, then migrations, then index DDL, and shipped
-- migration 003 references user_id by name at migration time. Migration 011
-- then renames user_id to subject_id, adds subject_kind/cost_microdollars,
-- swaps the unique to ai_quota_buckets_subject_key, and creates the subject
-- index — on fresh and existing databases alike.
CREATE TABLE IF NOT EXISTS ai_quota_buckets (
    id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    window_seconds INTEGER NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, window_seconds, window_start)
);
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_window ON ai_quota_buckets(window_start);
