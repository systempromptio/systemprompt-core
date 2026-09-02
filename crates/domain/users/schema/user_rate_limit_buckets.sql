CREATE TABLE IF NOT EXISTS user_rate_limit_buckets (
    user_id VARCHAR(255) NOT NULL,
    scope TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    hits BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, scope, window_start)
);

CREATE INDEX IF NOT EXISTS idx_user_rate_limit_buckets_window
    ON user_rate_limit_buckets(window_start);
