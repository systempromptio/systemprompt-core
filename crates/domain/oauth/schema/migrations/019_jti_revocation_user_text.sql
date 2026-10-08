-- `oauth_jti_revocations.user_id` shipped as UUID while `users.id` is TEXT and
-- holds non-UUID ids (seeded admins, imported users, service accounts), so a
-- non-UUID user could not log out and a revoke for one was skipped. The column
-- becomes TEXT and references the user it belongs to. Revocations whose user
-- is gone protect nothing (the user cannot authenticate) and are deleted
-- first so the foreign key can be added.
DELETE FROM oauth_jti_revocations
WHERE user_id::text NOT IN (SELECT id FROM users);

ALTER TABLE oauth_jti_revocations
    DROP CONSTRAINT IF EXISTS oauth_jti_revocations_user_id_fkey;

ALTER TABLE oauth_jti_revocations
    ALTER COLUMN user_id TYPE TEXT USING user_id::text;

ALTER TABLE oauth_jti_revocations
    ADD CONSTRAINT oauth_jti_revocations_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
