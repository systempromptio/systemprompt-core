CREATE TABLE IF NOT EXISTS ai_gateway_policies (
    id TEXT PRIMARY KEY,
    tenant_id VARCHAR(255),
    name VARCHAR(255) NOT NULL,
    spec JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, name)
);
CREATE INDEX IF NOT EXISTS idx_ai_gateway_policies_tenant ON ai_gateway_policies(tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_gateway_policies_enabled ON ai_gateway_policies(enabled);
