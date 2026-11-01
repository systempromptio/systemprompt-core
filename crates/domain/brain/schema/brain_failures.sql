CREATE TABLE IF NOT EXISTS brain_failures (
    id TEXT PRIMARY KEY,
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT,
    stage TEXT NOT NULL,
    reason TEXT NOT NULL,
    attempt INT NOT NULL DEFAULT 1 CHECK (attempt > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_failures_outbox
    ON brain_failures (outbox_id);
CREATE INDEX IF NOT EXISTS idx_brain_failures_created
    ON brain_failures (created_at);
