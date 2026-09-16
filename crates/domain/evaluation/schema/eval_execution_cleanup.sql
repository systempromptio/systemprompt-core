CREATE TABLE IF NOT EXISTS eval_execution_cleanup (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    container_id TEXT,
    network_id TEXT,
    status TEXT NOT NULL CHECK(status IN ('pending','verified','failed','retrying')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
