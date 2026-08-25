-- Drop the UNIQUE constraint on users.name.
--
-- name is a display/login label, not an identity key — email (still UNIQUE)
-- is. The constraint forced federated and passkey sign-up paths to write the
-- email into name purely to avoid collisions, and two federated users sharing
-- a preferred_username could not coexist. Lookups by name resolve ties by
-- oldest account first (see find_by_name).

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_name_key;
