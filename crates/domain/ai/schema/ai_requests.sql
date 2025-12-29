CREATE TABLE IF NOT EXISTS ai_requests (
    id TEXT PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL UNIQUE,
    user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255),
    task_id VARCHAR(255),
    context_id VARCHAR(255),
    trace_id VARCHAR(255),
    mcp_execution_id VARCHAR(255),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    temperature DOUBLE PRECISION,
    top_p DOUBLE PRECISION,
    max_tokens INTEGER,
    stop_sequences TEXT,
    tokens_used INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_cents INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER,
    cache_hit BOOLEAN NOT NULL DEFAULT FALSE,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    is_streaming BOOLEAN NOT NULL DEFAULT FALSE,
    status VARCHAR(255) NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ai_requests_request_id ON ai_requests(request_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_provider ON ai_requests(provider);
CREATE INDEX IF NOT EXISTS idx_ai_requests_status ON ai_requests(status);
CREATE INDEX IF NOT EXISTS idx_ai_requests_created_at ON ai_requests(created_at);
CREATE INDEX IF NOT EXISTS idx_ai_requests_user_id ON ai_requests(user_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_session_id ON ai_requests(session_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_task_id ON ai_requests(task_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_context_id ON ai_requests(context_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_trace_id ON ai_requests(trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_mcp_execution_id ON ai_requests(mcp_execution_id);
CREATE INDEX IF NOT EXISTS idx_ai_requests_cost ON ai_requests(cost_cents);

CREATE INDEX IF NOT EXISTS idx_ai_requests_user_created ON ai_requests(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_ai_requests_user_model ON ai_requests(user_id, model);
CREATE INDEX IF NOT EXISTS idx_ai_requests_provider_status ON ai_requests(provider, status);
CREATE INDEX IF NOT EXISTS idx_ai_requests_session_created ON ai_requests(session_id, created_at);
