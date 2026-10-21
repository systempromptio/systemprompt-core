CREATE TABLE IF NOT EXISTS governance_rate_charges (
    call_id TEXT NOT NULL,
    bucket_key TEXT NOT NULL,
    position INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (call_id, bucket_key)
);

CREATE INDEX IF NOT EXISTS idx_governance_rate_charges_expires_at
    ON governance_rate_charges (expires_at);
