-- The migration bookkeeping table carried three indexes on one key:
-- `(extension_id)` inside `(extension_id, version)` inside the UNIQUE
-- constraint `extension_migrations_extension_id_version_key`, which has the
-- same columns again. Every extension install writes this table, so both
-- extra indexes are cost with no read they serve alone.
--
-- Neither is UNIQUE and neither backs a constraint.
DROP INDEX IF EXISTS idx_extension_migrations_ext_id;
DROP INDEX IF EXISTS idx_extension_migrations_ext_version;
