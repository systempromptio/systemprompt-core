-- Bring legacy access_control_rules tables in line with the current shape:
-- broadened entity_type/rule_type/access CHECKs and an optional justification.
ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook'));

ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_rule_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_rule_type_check
    CHECK (rule_type IN ('role','department','user'));

ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_access_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_access_check
    CHECK (access IN ('allow','deny'));

ALTER TABLE access_control_rules
    ADD COLUMN IF NOT EXISTS justification TEXT;
ALTER TABLE access_control_rules ALTER COLUMN justification DROP DEFAULT;
ALTER TABLE access_control_rules ALTER COLUMN justification DROP NOT NULL;
UPDATE access_control_rules SET justification = NULL WHERE justification = '';
