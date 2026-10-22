CREATE TABLE IF NOT EXISTS ai_gateway_policies (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    spec JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    priority INTEGER NOT NULL DEFAULT 0,
    -- Provenance in the closed vocabulary code:<file> | bundle:<name> |
    -- console:<user> | migration, stamped from the change context with the
    -- last writer (sp_stamp_change).
    source TEXT CONSTRAINT ai_gateway_policies_source_check
        CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$'),
    -- Last writer, from the transaction's change context (sp_stamp_change).
    changed_by TEXT,
    changed_at TIMESTAMPTZ,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (name)
);
CREATE INDEX IF NOT EXISTS idx_ai_gateway_policies_enabled ON ai_gateway_policies(enabled);
