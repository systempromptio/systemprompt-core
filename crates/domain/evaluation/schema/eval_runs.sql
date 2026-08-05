CREATE TABLE IF NOT EXISTS eval_runs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('judge', 'replay', 'pairwise')),
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    judge_provider TEXT NOT NULL,
    judge_model TEXT NOT NULL,
    filter JSONB NOT NULL DEFAULT '{}'::jsonb,
    sample_size INTEGER NOT NULL DEFAULT 0,
    scored_count INTEGER NOT NULL DEFAULT 0,
    failed_count INTEGER NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    rubric_id TEXT,
    trigger_source TEXT NOT NULL DEFAULT 'manual' CHECK (trigger_source IN ('scheduled', 'cli', 'manual'))
);
CREATE INDEX IF NOT EXISTS idx_eval_runs_created_at ON eval_runs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_eval_runs_kind ON eval_runs(kind);
