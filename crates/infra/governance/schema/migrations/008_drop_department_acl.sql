-- Remove department-as-rule from the core authz schema.
--
-- Before: access_control_rules.rule_type included a 'department' variant,
-- evaluated by the in-process resolver against AuthzRequest.department.
-- This baked one Boeing-specific identity axis into core.
--
-- After: rule_type is restricted to ('role','user'). Tenants that need
-- attribute-based rules (department, clearance, jurisdiction, ...) define
-- their own table and an extension AuthzDecisionHook composed alongside
-- the core RuleBasedHook via CompositeAuthzHook.
--
-- Existing department rule rows are removed; entity catalog rows are
-- preserved. Any role/user rules referencing the same entity continue to
-- evaluate normally.

DELETE FROM access_control_rules WHERE rule_type = 'department';

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_rule_type_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_rule_type_check
    CHECK (rule_type IN ('role','user'));
