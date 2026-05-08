CREATE TABLE IF NOT EXISTS access_control_rules (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    rule_value TEXT NOT NULL,
    access TEXT NOT NULL DEFAULT 'allow',
    default_included BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_type, entity_id, rule_type, rule_value)
);

ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook'));

ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_rule_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_rule_type_check
    CHECK (rule_type IN ('role','department','user'));

ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_access_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_access_check
    CHECK (access IN ('allow','deny'));

CREATE INDEX IF NOT EXISTS idx_acl_entity ON access_control_rules(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_acl_rule ON access_control_rules(rule_type, rule_value);
CREATE INDEX IF NOT EXISTS idx_acl_default ON access_control_rules(default_included) WHERE default_included = true;

-- Operator-supplied note explaining *why* the rule exists. Surfaced in the
-- access matrix tooltip and copied into `governance_decisions.evaluated_rules`
-- when a rule decides, so an auditor can see policy *intent* alongside the
-- rule that fired. NULL means "no operator reason given" — distinct from an
-- empty string.
ALTER TABLE access_control_rules
    ADD COLUMN IF NOT EXISTS justification TEXT;
ALTER TABLE access_control_rules ALTER COLUMN justification DROP DEFAULT;
ALTER TABLE access_control_rules ALTER COLUMN justification DROP NOT NULL;
UPDATE access_control_rules SET justification = NULL WHERE justification = '';
