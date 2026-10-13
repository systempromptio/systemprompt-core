CREATE TABLE IF NOT EXISTS agent_run_events (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('init', 'assistant_text', 'tool_use', 'tool_result', 'hook', 'result', 'stderr', 'runner')),
    tool_name TEXT,
    tool_use_id TEXT,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (run_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_agent_run_events_tool_use ON agent_run_events(run_id, tool_use_id);
