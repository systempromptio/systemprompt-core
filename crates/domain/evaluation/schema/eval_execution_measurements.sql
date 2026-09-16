CREATE TABLE IF NOT EXISTS eval_execution_measurements (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    hard_failures TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    deterministic_checks JSONB NOT NULL DEFAULT '{}'::JSONB,
    judgment JSONB,
    quality_milli INTEGER CHECK(quality_milli BETWEEN 0 AND 5000),
    latency_ms BIGINT CHECK(latency_ms>=0),
    input_tokens BIGINT CHECK(input_tokens>=0),
    output_tokens BIGINT CHECK(output_tokens>=0),
    tool_calls BIGINT CHECK(tool_calls>=0),
    attempted_cost_microdollars BIGINT NOT NULL DEFAULT 0 CHECK(attempted_cost_microdollars>=0),
    accounting_status TEXT NOT NULL CHECK(accounting_status IN ('complete','partial','unknown')),
    verified_success BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
