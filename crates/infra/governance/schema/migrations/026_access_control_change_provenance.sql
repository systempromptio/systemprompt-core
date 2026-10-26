-- Who last changed each access-control rule and entity, when and why, stamped
-- by `sp_stamp_change()` from the writer's transaction-local change context.
-- Existing rows predate the context and keep NULL: nothing is backfilled.
--
-- Why: lock footprint. Each ALTER takes ACCESS EXCLUSIVE on its table for a
-- catalog-only change (nullable columns, no default, no rewrite, no scan), so
-- the lock lasts milliseconds whatever the row count. Neither table is
-- `ai_requests`; that table is not touched.
ALTER TABLE access_control_rules
    ADD COLUMN IF NOT EXISTS changed_by TEXT,
    ADD COLUMN IF NOT EXISTS changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS reason TEXT;

ALTER TABLE access_control_entities
    ADD COLUMN IF NOT EXISTS changed_by TEXT,
    ADD COLUMN IF NOT EXISTS changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS reason TEXT;
