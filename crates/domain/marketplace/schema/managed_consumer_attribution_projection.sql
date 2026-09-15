CREATE TABLE IF NOT EXISTS managed_consumer_attribution_projection (
    evidence_id TEXT PRIMARY KEY REFERENCES managed_consumer_invocation_evidence(id) ON DELETE CASCADE,
    receipt_id TEXT REFERENCES managed_installation_receipts(id) ON DELETE SET NULL,
    version BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
