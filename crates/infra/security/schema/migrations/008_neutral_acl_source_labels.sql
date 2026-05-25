-- Rename pre-`007_split_acl_entities`-era `legacy:*` provenance labels on
-- `access_control_entities` to neutral, descriptive ones. The labels were
-- written by `007_split_acl_entities.sql` when promoting sentinel rows from
-- the legacy rule-encoded `default_included` scheme; that backfill is long
-- past, so the "legacy" prefix is now misleading runtime state.
--
-- Idempotent: rows already on the new labels are untouched.
UPDATE access_control_entities
   SET source = 'bootstrap:default_promoted',
       updated_at = NOW()
 WHERE source = 'legacy:sentinel';

UPDATE access_control_entities
   SET source = 'bootstrap:rule_derived',
       updated_at = NOW()
 WHERE source = 'legacy:rule-derived';
