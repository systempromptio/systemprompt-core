-- Extend the access_control_rules entity_type CHECK to include
-- 'gateway_model', the per-model RBAC entity that pairs with the existing
-- 'gateway_route'. EntityKind::GatewayModel was added in 0.11.2 so the
-- gateway dispatch path can authz per-model (in addition to per-route).
--
-- Idempotent: the constraint is dropped before being re-added.
ALTER TABLE access_control_rules DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','gateway_model','skill','hook'));
