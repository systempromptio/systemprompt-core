CREATE TABLE IF NOT EXISTS access_control_entities (
    entity_type TEXT NOT NULL
        CONSTRAINT access_control_entities_entity_type_check
        CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook')),
    entity_id TEXT NOT NULL,
    default_included BOOLEAN NOT NULL DEFAULT false,
    -- Provenance label: "profile:<name>" (publish-pipeline bootstrap),
    -- "roles.yaml" (access-control loader), or "bootstrap:*" for rows
    -- promoted from older schemas by a migration.
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_access_control_entities_default
    ON access_control_entities(default_included)
    WHERE default_included = true;
