CREATE TABLE IF NOT EXISTS brain_proposals (
    id TEXT PRIMARY KEY,
    analysis TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('config_diff', 'fact_actions')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'approved', 'rejected')),
    summary TEXT NOT NULL,
    body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    decided_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_brain_proposals_open
    ON brain_proposals (created_at DESC) WHERE status = 'open';
