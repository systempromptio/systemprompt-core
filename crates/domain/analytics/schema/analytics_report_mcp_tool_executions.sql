CREATE TABLE IF NOT EXISTS analytics_report_mcp_tool_executions (
    mcp_execution_id TEXT PRIMARY KEY,
    tool_name VARCHAR(255) NOT NULL,
    server_name VARCHAR(255) NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    execution_time_ms INTEGER,
    status VARCHAR(255) NOT NULL,
    error_message TEXT,
    user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255),
    context_id VARCHAR(255),
    task_id VARCHAR(255),
    trace_id VARCHAR(255),
    request_method TEXT,
    request_source TEXT,
    actor_kind TEXT,
    actor_id TEXT,
    ai_tool_call_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_started_at ON analytics_report_mcp_tool_executions (started_at);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_user_id ON analytics_report_mcp_tool_executions (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_session_id ON analytics_report_mcp_tool_executions (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_context_id ON analytics_report_mcp_tool_executions (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_task_id ON analytics_report_mcp_tool_executions (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_created_at ON analytics_report_mcp_tool_executions (created_at);
