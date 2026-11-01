CREATE TABLE IF NOT EXISTS brain_outcomes (
    id TEXT PRIMARY KEY,
    recall_id TEXT NOT NULL REFERENCES brain_recalls(id) ON DELETE CASCADE,
    fact_id TEXT NOT NULL,
    signal TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (recall_id, fact_id, signal)
);

CREATE INDEX IF NOT EXISTS idx_brain_outcomes_fact
    ON brain_outcomes (fact_id);
CREATE INDEX IF NOT EXISTS idx_brain_outcomes_created
    ON brain_outcomes (created_at);
