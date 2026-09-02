-- `webauthn_challenges` is listed in MERGE_EXCLUDED_SECURITY_TABLES, which
-- holds tables whose rows must die with the source identity on a user merge.
-- The table shipped with a bare `user_id` and no foreign key, so those rows
-- outlived the user they authenticated and the merge left orphaned auth state.
DELETE FROM webauthn_challenges
WHERE user_id IS NOT NULL
  AND user_id NOT IN (SELECT id FROM users);

ALTER TABLE webauthn_challenges
    DROP CONSTRAINT IF EXISTS webauthn_challenges_user_id_fkey;

ALTER TABLE webauthn_challenges
    ADD CONSTRAINT webauthn_challenges_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
