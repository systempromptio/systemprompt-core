-- Access control for the `access_control` config plane (credential.md §3.1).
--
-- 1. `model` and `provider` become entity kinds: the plane stores
--    `model/<glob>` and `provider/<name>` entitlements as written and the
--    resolver matches them at decision time.
-- 2. `source` moves to the one closed provenance vocabulary every plane uses:
--    `code:<file>`, `bundle:<name>`, `console:<user>` or `migration`. The old
--    labels map as follows: `yaml` (the baked tree) → `code:services`;
--    `roles.yaml` and `ingestion:access_control_config` (the deleted roles
--    file) → `code:access-control/rules.yaml`; `services:<file>` →
--    `code:<file>`; `marketplace:<id>` → `code:marketplaces/<id>`;
--    `dashboard` → `console:<the row's changed_by, else unknown>`; anything
--    else (`profile:*`, `bootstrap:*`, `slack:*`, `teams:*`) → `migration`.
--    `bundle:<name>` rows keep their value. The rules column loses its
--    `'yaml'` default, so a writer that names no origin fails loudly.
--
-- The rewrite runs under change context actor `migration`, so the
-- `sp_stamp_change` trigger records it as such.
--
-- Why: lock footprint. Both tables hold one row per declared entity and
-- grant (thousands, not millions); `ai_requests` is not touched. Each UPDATE
-- holds ROW EXCLUSIVE on its table for the rewrite; each constraint is added
-- NOT VALID under a brief ACCESS EXCLUSIVE and VALIDATEd under SHARE UPDATE
-- EXCLUSIVE, so concurrent reads and writes continue while it scans.
SELECT set_config('systemprompt.actor', 'migration', true),
       set_config('systemprompt.reason', 'governance 027: closed source vocabulary', true);

UPDATE access_control_rules
   SET source = CASE
        WHEN source = 'yaml' THEN 'code:services'
        WHEN source IN ('roles.yaml', 'ingestion:access_control_config')
            THEN 'code:access-control/rules.yaml'
        WHEN source = 'dashboard' THEN 'console:' || COALESCE(NULLIF(changed_by, ''), 'unknown')
        WHEN source LIKE 'services:_%' THEN 'code:' || substr(source, 10)
        WHEN source LIKE 'marketplace:_%' THEN 'code:marketplaces/' || substr(source, 13)
        ELSE 'migration'
    END
 WHERE source !~ '^(code:.+|bundle:.+|console:.+|migration)$';

UPDATE access_control_entities
   SET source = CASE
        WHEN source = 'yaml' THEN 'code:services'
        WHEN source IN ('roles.yaml', 'ingestion:access_control_config')
            THEN 'code:access-control/rules.yaml'
        WHEN source = 'dashboard' THEN 'console:' || COALESCE(NULLIF(changed_by, ''), 'unknown')
        WHEN source LIKE 'services:_%' THEN 'code:' || substr(source, 10)
        WHEN source LIKE 'marketplace:_%' THEN 'code:marketplaces/' || substr(source, 13)
        ELSE 'migration'
    END
 WHERE source !~ '^(code:.+|bundle:.+|console:.+|migration)$';

ALTER TABLE access_control_rules ALTER COLUMN source DROP DEFAULT;

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_entity_type_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill',
                           'hook','slack_workspace','teams_tenant','model','provider')) NOT VALID;
ALTER TABLE access_control_rules VALIDATE CONSTRAINT access_control_rules_entity_type_check;

ALTER TABLE access_control_entities
    DROP CONSTRAINT IF EXISTS access_control_entities_entity_type_check;
ALTER TABLE access_control_entities
    ADD CONSTRAINT access_control_entities_entity_type_check
    CHECK (entity_type IN ('plugin','agent','mcp_server','marketplace','gateway_route','skill',
                           'hook','slack_workspace','teams_tenant','model','provider')) NOT VALID;
ALTER TABLE access_control_entities VALIDATE CONSTRAINT access_control_entities_entity_type_check;

ALTER TABLE access_control_rules
    DROP CONSTRAINT IF EXISTS access_control_rules_source_check;
ALTER TABLE access_control_rules
    ADD CONSTRAINT access_control_rules_source_check
    CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$') NOT VALID;
ALTER TABLE access_control_rules VALIDATE CONSTRAINT access_control_rules_source_check;

ALTER TABLE access_control_entities
    DROP CONSTRAINT IF EXISTS access_control_entities_source_check;
ALTER TABLE access_control_entities
    ADD CONSTRAINT access_control_entities_source_check
    CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$') NOT VALID;
ALTER TABLE access_control_entities VALIDATE CONSTRAINT access_control_entities_source_check;
