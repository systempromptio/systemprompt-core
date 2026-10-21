-- Cluster-wide fixed-window counters for the `rate_limit` governance policy.
--
-- Every replica charges the same row, so the budget is per deployment rather
-- than per process. `governance_rate_charges` remembers the position each
-- call id was charged at: `GovernancePolicy::evaluate` is idempotent per call,
-- so a re-evaluated call reads its original position instead of charging again.
CREATE TABLE IF NOT EXISTS governance_rate_windows (
    bucket_key TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    hits INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket_key, window_start)
);

CREATE INDEX IF NOT EXISTS idx_governance_rate_windows_expires_at
    ON governance_rate_windows (expires_at);
