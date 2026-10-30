CREATE TABLE IF NOT EXISTS brain_event_results (
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    outcome TEXT CHECK (outcome IN (
        'processed', 'ignored', 'no_filter', 'rejected', 'failed', 'abandoned'
    )),
    filter TEXT,
    facts_written INT NOT NULL DEFAULT 0 CHECK (facts_written >= 0),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    PRIMARY KEY (outbox_id, pipeline_version)
);

CREATE INDEX IF NOT EXISTS idx_brain_event_results_claimed
    ON brain_event_results (claimed_at);
