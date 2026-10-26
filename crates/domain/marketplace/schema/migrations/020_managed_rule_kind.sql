-- @supersedes-checksum: 77cf1aed13c2040f
-- @supersedes-checksum: 404d8f29e1479ead
-- @cost: rows=157 measured=22ms triggers=live
-- Rules are a managed resource kind, so a rule's publication identity reaches
-- the signed manifest beside skills and plugins.
ALTER TABLE managed_resources DROP CONSTRAINT IF EXISTS managed_resources_kind_check;
ALTER TABLE managed_resources ADD CONSTRAINT managed_resources_kind_check
    CHECK (kind IN ('skill','plugin','marketplace','supporting','rule')) NOT VALID;
ALTER TABLE managed_resources VALIDATE CONSTRAINT managed_resources_kind_check;
