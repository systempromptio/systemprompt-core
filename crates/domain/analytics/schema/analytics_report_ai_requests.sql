CREATE TABLE IF NOT EXISTS analytics_report_ai_requests (
    id TEXT PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255),
    task_id TEXT,
    context_id VARCHAR(255) NOT NULL,
    gateway_conversation_id VARCHAR(255),
    client_session_id TEXT,
    provider_request_id VARCHAR(255),
    trace_id VARCHAR(255),
    mcp_execution_id VARCHAR(255),
    provider TEXT,
    model TEXT,
    requested_model TEXT,
    route_match TEXT,
    temperature DOUBLE PRECISION,
    top_p DOUBLE PRECISION,
    max_tokens INTEGER,
    tokens_used INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_microdollars BIGINT NOT NULL,
    latency_ms INTEGER,
    upstream_latency_ms INTEGER,
    cache_hit BOOLEAN NOT NULL,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    reasoning_tokens INTEGER,
    is_streaming BOOLEAN NOT NULL,
    status VARCHAR(255) NOT NULL,
    error_message TEXT,
    actor_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    synthetic BOOLEAN NOT NULL,
    request_kind TEXT NOT NULL,
    instance_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_request_id ON analytics_report_ai_requests (request_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_user_id ON analytics_report_ai_requests (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_session_id ON analytics_report_ai_requests (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_task_id ON analytics_report_ai_requests (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_context_id ON analytics_report_ai_requests (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_created_at ON analytics_report_ai_requests (created_at);
