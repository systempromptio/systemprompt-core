CREATE TABLE IF NOT EXISTS task_execution_steps (
    step_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    step_type TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    content JSONB NOT NULL DEFAULT '{}',
    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    duration_ms INTEGER,
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_execution_steps_task_id ON task_execution_steps(task_id);
CREATE INDEX IF NOT EXISTS idx_task_execution_steps_status ON task_execution_steps(status);
