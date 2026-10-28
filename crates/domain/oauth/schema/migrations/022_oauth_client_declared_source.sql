-- Identity plane (services/identity/*.yaml): the declaration file a static
-- OAuth client is managed by, and the secrets-document key its secret is read
-- from (the secret itself is only ever stored hashed). Both NULL for a client
-- registered at runtime. Nullable columns with no default are catalog-only
-- changes. Same DDL as schema/oauth_clients.sql.
ALTER TABLE oauth_clients ADD COLUMN IF NOT EXISTS declared_source TEXT;
ALTER TABLE oauth_clients ADD COLUMN IF NOT EXISTS declared_secret_key TEXT;
