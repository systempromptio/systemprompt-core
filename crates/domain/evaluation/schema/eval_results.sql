CREATE TABLE IF NOT EXISTS eval_results (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES eval_runs(id) ON DELETE CASCADE,
    ai_request_id TEXT,
    case_id TEXT REFERENCES eval_cases(id) ON DELETE SET NULL,
    user_id TEXT,
    session_id TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    overall_score INTEGER CHECK (overall_score BETWEEN 1 AND 5),
    dimension_scores JSONB NOT NULL DEFAULT '{}'::jsonb,
    verdict TEXT NOT NULL CHECK (verdict IN ('pass', 'partial', 'fail', 'skipped')),
    rationale TEXT,
    repair_hint TEXT,
    flags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    prompt_excerpt TEXT,
    response_excerpt TEXT,
    latency_ms INTEGER,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    judge_cost_microdollars BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    repaired BOOLEAN NOT NULL DEFAULT FALSE,
    replay_of_result_id TEXT,
    judge_ai_request_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_eval_results_run ON eval_results(run_id);
CREATE INDEX IF NOT EXISTS idx_eval_results_model ON eval_results(model);
CREATE INDEX IF NOT EXISTS idx_eval_results_request ON eval_results(ai_request_id);
CREATE INDEX IF NOT EXISTS idx_eval_results_case ON eval_results(case_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_eval_results_run_request
    ON eval_results(run_id, ai_request_id)
    WHERE ai_request_id IS NOT NULL AND replay_of_result_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_eval_results_replay_of ON eval_results(replay_of_result_id);
