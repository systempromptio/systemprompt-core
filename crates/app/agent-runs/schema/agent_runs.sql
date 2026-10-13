CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    origin_kind TEXT NOT NULL,
    origin_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    host TEXT NOT NULL
        CHECK (host IN ('claude-code', 'codex-cli', 'opencode', 'hermes')),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'preparing', 'running', 'succeeded', 'failed', 'cancelled', 'timed_out')),
    request JSONB NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    not_before TIMESTAMPTZ,
    claimed_by TEXT,
    lease_until TIMESTAMPTZ,
    claimed_at TIMESTAMPTZ,
    run_dir TEXT,
    gateway_session_id TEXT REFERENCES user_sessions(session_id) ON DELETE SET NULL,
    api_key_id TEXT,
    host_session_id TEXT,
    pid INTEGER,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration_ms BIGINT,
    error_class TEXT
        CHECK (error_class IN ('ok', 'budget_exceeded', 'timeout', 'output_invalid', 'hook_denied', 'host_crashed', 'connector_unavailable')),
    turns INTEGER,
    input_tokens BIGINT,
    output_tokens BIGINT,
    cost_microdollars BIGINT,
    structured_output JSONB,
    exit_code INTEGER,
    error TEXT,
    diff_summary JSONB,
    cancel_requested_at TIMESTAMPTZ,
    task_id TEXT,
    context_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_runs_claim ON agent_runs(status, not_before, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_runs_lease ON agent_runs(lease_until)
    WHERE status IN ('preparing', 'running');
CREATE INDEX IF NOT EXISTS idx_agent_runs_origin ON agent_runs(origin_kind, origin_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_user_created ON agent_runs(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_runs_gateway_session ON agent_runs(gateway_session_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_task ON agent_runs(task_id) WHERE task_id IS NOT NULL;

CREATE OR REPLACE TRIGGER update_agent_runs_updated_at
    BEFORE UPDATE ON agent_runs
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp_trigger();
