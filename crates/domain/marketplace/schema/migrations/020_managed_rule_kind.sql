-- Rules are a managed resource kind, so a rule's publication identity reaches
-- the signed manifest beside skills and plugins.
ALTER TABLE managed_resources DROP CONSTRAINT IF EXISTS managed_resources_kind_check;
ALTER TABLE managed_resources ADD CONSTRAINT managed_resources_kind_check
    CHECK (kind IN ('skill','plugin','marketplace','supporting','rule'));
