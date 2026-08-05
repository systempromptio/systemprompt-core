CREATE TABLE IF NOT EXISTS eval_judge_calls (
    conversation_id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES eval_runs(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    result_id TEXT,
    judge_ai_request_id TEXT,
    rubric_id TEXT,
    cost_microdollars BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_eval_judge_calls_run ON eval_judge_calls(run_id);
