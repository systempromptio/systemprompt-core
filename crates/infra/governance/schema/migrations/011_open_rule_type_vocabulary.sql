-- Open access_control_rules.rule_type to extension-declared subject dimensions.
--
-- Migration 008 closed rule_type to ('role','user') on the reasoning that a
-- tenant needing department-style rules should stand up its own table and an
-- ABAC hook. In practice tenants kept writing department rows into this table
-- (the admin access matrix reads and manages them), and the CHECK was the only
-- thing standing between the declared schema and reality.
--
-- The resolver now takes the extensible route instead: extensions declare a
-- SubjectDimension, register a SubjectAttributeProvider, and their rule_type
-- slug is interleaved into the precedence ladder. Core still ships only 'user'
-- and 'role'; the column becomes an open vocabulary validated by
-- authz::RuleType at the Rust boundary, exactly as AuthzContext.kind is.
--
-- entity_type keeps its CHECK: it is a closed core enum (EntityKind).

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_rule_type_check;
