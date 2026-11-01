CREATE TABLE IF NOT EXISTS brain_pipeline_versions (
    version TEXT PRIMARY KEY CHECK (version ~ '^[0-9a-f]{16}$'),
    declared_hash TEXT NOT NULL,
    code_version INT NOT NULL CHECK (code_version > 0),
    prompt_versions JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_pipeline_versions_recorded
    ON brain_pipeline_versions (recorded_at DESC);
