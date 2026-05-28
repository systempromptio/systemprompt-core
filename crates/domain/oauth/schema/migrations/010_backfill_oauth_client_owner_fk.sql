BEGIN;

DELETE FROM oauth_clients
 WHERE owner_user_id IS NULL OR owner_user_id NOT IN (SELECT id FROM users);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'oauth_clients_owner_user_id_fkey'
       AND conrelid = 'oauth_clients'::regclass
  ) THEN
    ALTER TABLE oauth_clients
      ADD CONSTRAINT oauth_clients_owner_user_id_fkey
      FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE;
  END IF;
END $$;

COMMIT;
