CREATE TABLE IF NOT EXISTS headless_run_events (
    run_id TEXT NOT NULL REFERENCES headless_runs(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('init', 'assistant_text', 'tool_use', 'tool_result', 'hook', 'result', 'stderr', 'runner')),
    tool_name TEXT,
    tool_use_id TEXT,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (run_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_headless_run_events_tool_use ON headless_run_events(run_id, tool_use_id);
