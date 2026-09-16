CREATE TABLE IF NOT EXISTS eval_execution_evidence (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    fencing_token BIGINT NOT NULL,
    digest TEXT NOT NULL,
    manifest JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
