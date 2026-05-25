-- Idempotent: drop the CHECK before re-adding so the migration can replay
-- against a partially-migrated DB without erroring.
ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','gateway_model','skill','hook'));
