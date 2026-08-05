CREATE TABLE IF NOT EXISTS eval_pairs (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES eval_runs(id) ON DELETE CASCADE,
    case_id TEXT REFERENCES eval_cases(id) ON DELETE SET NULL,
    model_a TEXT NOT NULL,
    model_b TEXT NOT NULL,
    winner TEXT NOT NULL CHECK (winner IN ('a', 'b', 'tie')),
    order_swapped BOOLEAN NOT NULL DEFAULT FALSE,
    rationale TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_eval_pairs_run ON eval_pairs(run_id);
CREATE INDEX IF NOT EXISTS idx_eval_pairs_case ON eval_pairs(case_id);
