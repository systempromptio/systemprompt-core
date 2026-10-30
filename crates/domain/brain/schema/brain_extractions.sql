CREATE TABLE IF NOT EXISTS brain_extractions (
    id TEXT PRIMARY KEY,
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    extractor TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT,
    prompt_version TEXT,
    raw_output JSONB NOT NULL,
    input_tokens INT NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens INT NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    candidates INT NOT NULL DEFAULT 0 CHECK (candidates >= 0),
    ai_request_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (outbox_id, pipeline_version, extractor, kind)
);

CREATE INDEX IF NOT EXISTS idx_brain_extractions_created
    ON brain_extractions (created_at);
