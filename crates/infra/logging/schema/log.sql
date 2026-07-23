CREATE TABLE IF NOT EXISTS logs (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    level VARCHAR(50) NOT NULL CHECK (level IN ('ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE')),
    module VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    metadata TEXT,
    user_id VARCHAR(255),
    session_id VARCHAR(255),
    task_id VARCHAR(255),
    trace_id VARCHAR(255),
    context_id VARCHAR(255),
    gateway_conversation_id VARCHAR(255),
    provider_request_id VARCHAR(255),
    client_id VARCHAR(255),
    CONSTRAINT log_level_check CHECK (level IN ('ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'))
);
-- Single-column level/user_id/session_id/context_id/client_id indexes omitted:
-- each is covered by a (col, timestamp DESC) composite below. See migration 003.
CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_module ON logs(module);
CREATE INDEX IF NOT EXISTS idx_logs_level_timestamp ON logs(level, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_task_id ON logs(task_id);
CREATE INDEX IF NOT EXISTS idx_logs_trace_id ON logs(trace_id);
CREATE INDEX IF NOT EXISTS idx_logs_gateway_conversation_id ON logs(gateway_conversation_id);
CREATE INDEX IF NOT EXISTS idx_logs_provider_request_id ON logs(provider_request_id);
CREATE INDEX IF NOT EXISTS idx_logs_user_timestamp ON logs(user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_session_timestamp ON logs(session_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_context_timestamp ON logs(context_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_client_timestamp ON logs(client_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_client_level ON logs(client_id, level);
