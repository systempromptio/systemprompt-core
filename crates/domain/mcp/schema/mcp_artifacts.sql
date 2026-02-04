CREATE TABLE IF NOT EXISTS mcp_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    artifact_id VARCHAR(255) NOT NULL UNIQUE,
    mcp_execution_id VARCHAR(255) NOT NULL,
    context_id VARCHAR(255),
    user_id VARCHAR(255),
    server_name VARCHAR(255) NOT NULL,
    artifact_type VARCHAR(100) NOT NULL,
    title VARCHAR(500),
    data JSONB NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_artifact_id ON mcp_artifacts(artifact_id);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_mcp_execution_id ON mcp_artifacts(mcp_execution_id);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_server_name ON mcp_artifacts(server_name);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_artifact_type ON mcp_artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_created_at ON mcp_artifacts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_expires_at ON mcp_artifacts(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_user_id ON mcp_artifacts(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_context_id ON mcp_artifacts(context_id) WHERE context_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_server_created ON mcp_artifacts(server_name, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_type_created ON mcp_artifacts(artifact_type, created_at DESC);
