-- Two governance-plane indexes are strict prefixes of a non-partial superset
-- on the same table. `governance_decisions` takes a row on every tool call,
-- so a redundant index there is a write on the hot path.
--
-- Neither is UNIQUE and neither backs a constraint.

-- covered by access_control_rules_entity_type_entity_id_rule_type_rule_v_key
DROP INDEX IF EXISTS idx_acl_entity;
-- covered by idx_governance_decisions_rate_limit (session_id, ...)
DROP INDEX IF EXISTS idx_governance_decisions_session;
