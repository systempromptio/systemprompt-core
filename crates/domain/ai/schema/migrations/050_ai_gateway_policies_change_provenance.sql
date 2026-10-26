-- Who last changed each gateway policy, when, why and from which origin,
-- stamped by `sp_stamp_change()` from the writer's transaction-local change
-- context. `source` is the closed vocabulary code:<file> | bundle:<name> |
-- console:<user> | migration. Existing rows keep NULL: nothing is backfilled.
--
-- Why: lock footprint. The ALTER takes ACCESS EXCLUSIVE for a catalog-only
-- change (nullable columns, no default, no rewrite). The CHECK is added NOT
-- VALID (catalog only) and validated separately under SHARE UPDATE EXCLUSIVE,
-- which scans `ai_gateway_policies` (tens of rows) without blocking writers.
-- `ai_requests` is not touched.
ALTER TABLE ai_gateway_policies
    ADD COLUMN IF NOT EXISTS source TEXT,
    ADD COLUMN IF NOT EXISTS changed_by TEXT,
    ADD COLUMN IF NOT EXISTS changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS reason TEXT;

ALTER TABLE ai_gateway_policies DROP CONSTRAINT IF EXISTS ai_gateway_policies_source_check;
ALTER TABLE ai_gateway_policies
    ADD CONSTRAINT ai_gateway_policies_source_check
    CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$') NOT VALID;
ALTER TABLE ai_gateway_policies VALIDATE CONSTRAINT ai_gateway_policies_source_check;
