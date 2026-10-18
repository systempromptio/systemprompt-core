-- Split access_control_rules into a per-entity catalog (access_control_entities)
-- plus the per-(entity, subject) grant table that retains the AccessRule shape.
--
-- Before: every entity's `default_included` flag was encoded as a sentinel row
-- (rule_type='role', rule_value='__default__') inside access_control_rules. The
-- column also lived on every real grant row, where it was meaningless. The
-- resolver had to special-case the sentinel and the publish pipeline could not
-- distinguish an "unknown" entity (no rows at all) from a "no grants" entity
-- (rows but every one denies).
--
-- After: access_control_entities owns one row per (entity_type, entity_id) and
-- carries the default_included flag and a source string ("profile:<name>",
-- "roles.yaml", etc). access_control_rules drops default_included entirely and
-- gets an FK back to the catalog. The resolver's ResolveInput now takes
-- default_included: Option<bool> — None means UnknownEntity (no catalog row),
-- Some(false) means NotAssigned (catalog row exists, no grant matched).

-- Drop the legacy gateway_model entity_type from any pre-existing rows.
-- EntityKind::GatewayModel was removed in the single-pass gateway authz
-- refactor; the new check constraint on access_control_entities below does
-- not include it, so leaving the rows in place would deadlock the INSERT
-- below (bootstrap:rule_derived sweep) and the FK validation later.
DELETE FROM access_control_rules WHERE entity_type = 'gateway_model';

-- Rebuild the rules check constraint to match the entity-catalog whitelist.
-- Idempotent: DROP IF EXISTS guards the replay path.
ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook'));

CREATE TABLE IF NOT EXISTS access_control_entities (
    entity_type TEXT NOT NULL
        CONSTRAINT access_control_entities_entity_type_check
        CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill','hook')),
    entity_id TEXT NOT NULL,
    default_included BOOLEAN NOT NULL DEFAULT false,
    -- Provenance label: "profile:<name>" (publish-pipeline bootstrap),
    -- "roles.yaml" / "departments.yaml" (access-control loader), or
    -- "bootstrap:*" for rows promoted from older schemas by a migration.
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_access_control_entities_default
    ON access_control_entities(default_included)
    WHERE default_included = true;

-- Promote every sentinel row into the catalog, preserving default_included.
-- Idempotent: ON CONFLICT DO NOTHING + WHERE column-exists guard so the
-- block is a no-op once `default_included` has been dropped from the rules
-- table on a replay.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'access_control_rules'
          AND column_name = 'default_included'
    ) THEN
        EXECUTE $sql$
            INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
            SELECT entity_type, entity_id, BOOL_OR(default_included), 'bootstrap:default_promoted'
            FROM access_control_rules
            WHERE rule_type = 'role' AND rule_value = '__default__'
            GROUP BY entity_type, entity_id
            ON CONFLICT (entity_type, entity_id) DO NOTHING
        $sql$;
    END IF;
END$$;

-- Seed catalog rows for every entity that has a real grant but no sentinel —
-- otherwise the FK below would reject existing rules and any future
-- list_rules_for_entity call would have nothing to anchor default_included to.
INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
SELECT DISTINCT entity_type, entity_id, false, 'bootstrap:rule_derived'
FROM access_control_rules
WHERE NOT (rule_type = 'role' AND rule_value = '__default__')
ON CONFLICT (entity_type, entity_id) DO NOTHING;

DELETE FROM access_control_rules
WHERE rule_type = 'role' AND rule_value = '__default__';

DROP INDEX IF EXISTS idx_acl_default;
ALTER TABLE access_control_rules DROP COLUMN IF EXISTS default_included;

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_entity_fk;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_entity_fk
    FOREIGN KEY (entity_type, entity_id)
    REFERENCES access_control_entities(entity_type, entity_id)
    ON DELETE CASCADE
    NOT VALID;
ALTER TABLE access_control_rules VALIDATE CONSTRAINT access_control_rules_entity_fk;
