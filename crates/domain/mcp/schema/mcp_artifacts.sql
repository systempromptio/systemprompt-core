-- One typed artifact per tool execution, from every vantage point.
--
-- `data` is the stored `ToolResponse` envelope (artifact + execution
-- metadata); the artifact body itself is content-addressed through
-- `payload_sha256`. `source` names where the platform saw the result and
-- `ai_tool_call_id` is the client `tool_use_id`, the one key that joins
-- intent (`ai_request_tool_calls`), execution (`mcp_tool_executions`) and
-- this result.
CREATE TABLE IF NOT EXISTS mcp_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    artifact_id VARCHAR(255) NOT NULL UNIQUE,
    mcp_execution_id VARCHAR(255) NOT NULL,
    context_id VARCHAR(255),
    user_id VARCHAR(255),
    session_id VARCHAR(255),
    trace_id VARCHAR(255),
    ai_tool_call_id VARCHAR(255),
    server_name VARCHAR(255) NOT NULL,
    tool_name VARCHAR(255),
    artifact_type VARCHAR(100) NOT NULL,
    title VARCHAR(500),
    source VARCHAR(32) NOT NULL DEFAULT 'in_process'
        CHECK (source IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode')),
    last_seen_source VARCHAR(32),
    data JSONB NOT NULL,
    metadata JSONB,
    payload_sha256 CHAR(64),
    payload_bytes INTEGER,
    is_structured BOOLEAN NOT NULL DEFAULT FALSE,
    has_ui_resource BOOLEAN NOT NULL DEFAULT FALSE,
    is_error BOOLEAN NOT NULL DEFAULT FALSE,
    secret_redactions INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ,
    FOREIGN KEY (mcp_execution_id) REFERENCES mcp_tool_executions(mcp_execution_id) ON DELETE CASCADE,
    FOREIGN KEY (payload_sha256) REFERENCES artifact_payloads(sha256) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_artifact_id ON mcp_artifacts(artifact_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_mcp_artifacts_execution ON mcp_artifacts(mcp_execution_id);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_server_name ON mcp_artifacts(server_name);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_artifact_type ON mcp_artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_created_at ON mcp_artifacts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_expires_at ON mcp_artifacts(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_user_id ON mcp_artifacts(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_context_id ON mcp_artifacts(context_id) WHERE context_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_server_created ON mcp_artifacts(server_name, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_type_created ON mcp_artifacts(artifact_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_session_created ON mcp_artifacts(session_id, created_at DESC) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_trace ON mcp_artifacts(trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_ai_tool_call ON mcp_artifacts(ai_tool_call_id) WHERE ai_tool_call_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_structured ON mcp_artifacts(created_at DESC) WHERE is_structured;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_payload ON mcp_artifacts(payload_sha256) WHERE payload_sha256 IS NOT NULL;
