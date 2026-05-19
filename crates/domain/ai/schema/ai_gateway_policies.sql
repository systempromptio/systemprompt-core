CREATE TABLE IF NOT EXISTS ai_gateway_policies (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    spec JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (name)
);
CREATE INDEX IF NOT EXISTS idx_ai_gateway_policies_enabled ON ai_gateway_policies(enabled);
