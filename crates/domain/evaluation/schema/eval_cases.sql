CREATE TABLE IF NOT EXISTS eval_cases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt_body JSONB NOT NULL,
    source_ai_request_id TEXT,
    expectation TEXT,
    baseline_response JSONB,
    baseline_model TEXT,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    repair_hint TEXT,
    canonical_messages JSONB,
    system_prompt TEXT,
    offered_tools JSONB,
    provider TEXT,
    model TEXT,
    prepared_body_sha256 TEXT
);
CREATE INDEX IF NOT EXISTS idx_eval_cases_enabled ON eval_cases(enabled);
CREATE INDEX IF NOT EXISTS idx_eval_cases_source ON eval_cases(source_ai_request_id);
