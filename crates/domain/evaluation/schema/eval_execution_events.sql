CREATE TABLE IF NOT EXISTS eval_execution_events (
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    sequence BIGINT NOT NULL CHECK(sequence>=0),
    payload JSONB NOT NULL,
    digest TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(execution_id,sequence)
);
