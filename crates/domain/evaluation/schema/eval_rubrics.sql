CREATE TABLE IF NOT EXISTS eval_rubrics (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    dimensions JSONB NOT NULL DEFAULT '[]'::jsonb,
    pass_threshold INTEGER NOT NULL DEFAULT 4 CHECK (pass_threshold BETWEEN 1 AND 5),
    prompt_template TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_eval_rubrics_enabled ON eval_rubrics(enabled);
