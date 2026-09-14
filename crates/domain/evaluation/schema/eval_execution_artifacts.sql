CREATE TABLE IF NOT EXISTS eval_execution_artifacts (
    execution_id TEXT PRIMARY KEY REFERENCES eval_execution_evidence(execution_id),
    content JSONB NOT NULL
);
