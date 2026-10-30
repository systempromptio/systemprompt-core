CREATE TABLE IF NOT EXISTS brain_config_docs (
    id TEXT PRIMARY KEY CHECK (id IN ('filters', 'importance', 'retrieval')),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
