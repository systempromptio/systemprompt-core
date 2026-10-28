-- Identity plane (services/identity/*.yaml): the declaration file a user row
-- is managed by, NULL for a row the console, SSO or the CLI created. Only
-- declared rows are read and written by the plane, so a row nobody declared is
-- never taken over or removed by `config apply`. A nullable column with no
-- default is a catalog-only change. Same DDL as schema/users.sql.
ALTER TABLE users ADD COLUMN IF NOT EXISTS declared_source TEXT;
