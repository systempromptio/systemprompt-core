-- `document` and `folder` become access-control entity kinds.
--
-- Why: lock footprint. Each constraint is added NOT VALID under a brief
-- ACCESS EXCLUSIVE and VALIDATEd under SHARE UPDATE EXCLUSIVE, so concurrent
-- reads and writes continue while it scans.
ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill',
                           'hook','slack_workspace','teams_tenant','model','provider',
                           'document','folder')) NOT VALID;
ALTER TABLE access_control_rules VALIDATE CONSTRAINT access_control_rules_entity_type_check;

ALTER TABLE access_control_entities
    DROP CONSTRAINT IF EXISTS access_control_entities_entity_type_check;
ALTER TABLE access_control_entities
    ADD CONSTRAINT access_control_entities_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill',
                           'hook','slack_workspace','teams_tenant','model','provider',
                           'document','folder')) NOT VALID;
ALTER TABLE access_control_entities VALIDATE CONSTRAINT access_control_entities_entity_type_check;
