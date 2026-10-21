-- Cluster-wide counters for the `rate_limit` governance policy, replacing the
-- per-process sliding window. See schema/governance_rate_windows.sql.
CREATE TABLE IF NOT EXISTS governance_rate_windows (
    bucket_key TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    hits INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket_key, window_start)
);

CREATE INDEX IF NOT EXISTS idx_governance_rate_windows_expires_at
    ON governance_rate_windows (expires_at);

CREATE TABLE IF NOT EXISTS governance_rate_charges (
    call_id TEXT NOT NULL,
    bucket_key TEXT NOT NULL,
    position INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (call_id, bucket_key)
);

CREATE INDEX IF NOT EXISTS idx_governance_rate_charges_expires_at
    ON governance_rate_charges (expires_at);
