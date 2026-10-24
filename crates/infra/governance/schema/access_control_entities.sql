CREATE TABLE IF NOT EXISTS access_control_entities (
    entity_type TEXT NOT NULL
        CONSTRAINT access_control_entities_entity_type_check
        CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook','slack_workspace','teams_tenant','model','provider')),
    entity_id TEXT NOT NULL,
    default_included BOOLEAN NOT NULL DEFAULT false,
    -- Provenance, one closed vocabulary: 'code:<file>', 'bundle:<name>',
    -- 'console:<user>' or 'migration' (see access_control_rules.source).
    source TEXT NOT NULL
        CONSTRAINT access_control_entities_source_check
        CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$'),
    -- Last writer, from the transaction's change context (sp_stamp_change).
    changed_by TEXT,
    changed_at TIMESTAMPTZ,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_access_control_entities_default
    ON access_control_entities(default_included)
    WHERE default_included = true;
