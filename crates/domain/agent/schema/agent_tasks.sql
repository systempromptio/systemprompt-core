CREATE TABLE IF NOT EXISTS agent_tasks (
    task_id TEXT PRIMARY KEY NOT NULL,

    context_id TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'submitted' CHECK (
        status IN (
            'submitted', 'working', 'input-required', 'completed',
            'canceled', 'failed', 'rejected', 'auth-required', 'unknown'
        )
    ),
    status_timestamp TIMESTAMPTZ,

    user_id TEXT,
    session_id TEXT,
    trace_id TEXT,
    agent_name TEXT,

    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    execution_time_ms INTEGER,
    error_message TEXT,

    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (context_id) REFERENCES user_contexts(context_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_context_id ON agent_tasks(context_id);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_status ON agent_tasks(status);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_status_timestamp ON agent_tasks(status_timestamp);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_created_at ON agent_tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_updated_at ON agent_tasks(updated_at);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_context_status ON agent_tasks(context_id, status);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_user_id ON agent_tasks(user_id);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_session_id ON agent_tasks(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_trace_id ON agent_tasks(trace_id);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_user_created ON agent_tasks(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_agent_name ON agent_tasks(agent_name);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_started_at ON agent_tasks(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_completed_at ON agent_tasks(completed_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_tasks_execution_time ON agent_tasks(execution_time_ms DESC);

CREATE INDEX IF NOT EXISTS idx_agent_tasks_error_message ON agent_tasks(error_message) WHERE error_message IS NOT NULL;

DROP TRIGGER IF EXISTS update_agent_tasks_updated_at ON agent_tasks;
CREATE TRIGGER update_agent_tasks_updated_at
    BEFORE UPDATE ON agent_tasks
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp_trigger();
