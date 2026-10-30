CREATE TABLE IF NOT EXISTS brain_disputes (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    candidate JSONB NOT NULL,
    similarity DOUBLE PRECISION,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolution TEXT,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_brain_disputes_fact
    ON brain_disputes (fact_id);
CREATE INDEX IF NOT EXISTS idx_brain_disputes_open
    ON brain_disputes (opened_at) WHERE status = 'open';
