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
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_user ON ai_quota_buckets(user_id);
CREATE INDEX IF NOT EXISTS idx_ai_quota_buckets_window ON ai_quota_buckets(window_start);
