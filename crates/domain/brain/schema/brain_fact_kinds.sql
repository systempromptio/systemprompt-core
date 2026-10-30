CREATE TABLE IF NOT EXISTS brain_fact_kinds (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
