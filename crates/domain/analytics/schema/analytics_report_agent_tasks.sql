CREATE TABLE IF NOT EXISTS analytics_report_agent_tasks (
    task_id TEXT PRIMARY KEY,
    context_id TEXT NOT NULL,
    status TEXT NOT NULL,
    status_timestamp TIMESTAMPTZ,
    user_id TEXT,
    session_id TEXT,
    trace_id TEXT,
    agent_name TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    execution_time_ms INTEGER,
    error_message TEXT,
    version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_context_id ON analytics_report_agent_tasks (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_user_id ON analytics_report_agent_tasks (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_session_id ON analytics_report_agent_tasks (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_started_at ON analytics_report_agent_tasks (started_at);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_created_at ON analytics_report_agent_tasks (created_at);
