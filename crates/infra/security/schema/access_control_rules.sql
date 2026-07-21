CREATE TABLE IF NOT EXISTS access_control_rules (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    entity_type TEXT NOT NULL
        CONSTRAINT access_control_rules_entity_type_check
        CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook','slack_workspace','teams_tenant')),
    entity_id TEXT NOT NULL,
    -- Open vocabulary, validated at the Rust boundary by authz::RuleType (the
    -- same stance AuthzContext.kind takes). Core mints 'user' and 'role';
    -- extensions mint their own lowercase snake_case dimension slugs
    -- ('department', 'cost_centre', ...) and teach the resolver about them by
    -- registering a SubjectAttributeProvider. A CHECK here would mean every
    -- new tenant dimension needed a core migration.
    rule_type TEXT NOT NULL,
    rule_value TEXT NOT NULL,
    access TEXT NOT NULL DEFAULT 'allow'
        CONSTRAINT access_control_rules_access_check
        CHECK (access IN ('allow','deny')),
    -- Operator-supplied note explaining *why* the rule exists. Surfaced in the
    -- access matrix tooltip and copied into governance_decisions.evaluated_rules
    -- when a rule decides. NULL is distinct from empty string.
    justification TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_type, entity_id, rule_type, rule_value),
    -- Every grant must be anchored to a catalog row in access_control_entities.
    -- The publish pipeline upserts entities ahead of the YAML loader so this FK
    -- is never racing with a fresh ingest.
    CONSTRAINT access_control_rules_entity_fk
        FOREIGN KEY (entity_type, entity_id)
        REFERENCES access_control_entities(entity_type, entity_id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_acl_entity ON access_control_rules(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_acl_rule ON access_control_rules(rule_type, rule_value);
