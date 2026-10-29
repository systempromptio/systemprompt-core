-- The release (brand) version a bridge reports alongside its wire version, and
-- whether its user has switched it off. Both NULL for a bridge that does not
-- report them. Nullable columns with no default are catalog-only changes. Same
-- DDL as schema/bridge_sessions.sql.
ALTER TABLE bridge_sessions ADD COLUMN IF NOT EXISTS release_version TEXT;
ALTER TABLE bridge_sessions ADD COLUMN IF NOT EXISTS state TEXT;
