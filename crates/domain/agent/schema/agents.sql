CREATE TABLE IF NOT EXISTS agents (
    agent_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '1.0.0',
    system_prompt TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    port INTEGER NOT NULL,
    endpoint TEXT NOT NULL,
    dev_only BOOLEAN NOT NULL DEFAULT false,
    is_primary BOOLEAN NOT NULL DEFAULT false,
    is_default BOOLEAN NOT NULL DEFAULT false,
    tags TEXT[],
    category_id TEXT,
    source_id TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    mcp_servers TEXT[],
    skills TEXT[],
    card_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agents_enabled ON agents(enabled);
CREATE INDEX IF NOT EXISTS idx_agents_source ON agents(source_id);
CREATE INDEX IF NOT EXISTS idx_agents_name ON agents(name);
