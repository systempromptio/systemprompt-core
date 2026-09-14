CREATE TABLE IF NOT EXISTS managed_reconciliation_conflicts (
    reconciliation_id TEXT NOT NULL REFERENCES managed_reconciliations(id),
    path TEXT NOT NULL,
    base_digest TEXT,
    candidate_digest TEXT,
    incoming_digest TEXT,
    resolution TEXT CHECK (resolution IN ('candidate','incoming','manual','delete')),
    resolved_digest TEXT,
    PRIMARY KEY(reconciliation_id, path)
);
