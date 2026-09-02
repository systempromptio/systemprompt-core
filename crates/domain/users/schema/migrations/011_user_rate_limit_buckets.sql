-- Replica-shared HTTP rate-limit counters keyed by verified user identity.
--
-- The in-process governor limits per replica, so N replicas grant N times the
-- configured budget to one caller. This table is the atomic counter behind
-- the global user budget; windows are pruned by window_start.

CREATE TABLE IF NOT EXISTS user_rate_limit_buckets (
    user_id VARCHAR(255) NOT NULL,
    scope TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    hits BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, scope, window_start)
);

CREATE INDEX IF NOT EXISTS idx_user_rate_limit_buckets_window
    ON user_rate_limit_buckets(window_start);
