-- Normalise users.email to its canonical form (trimmed, lowercased) so one
-- human resolves to one account across auth paths. Rows differing only by
-- case must be merged by an operator first — auto-merging would rekey rows
-- across ~20 tables without review, so the migration fails loudly instead.
DO $$
DECLARE
    collision RECORD;
BEGIN
    SELECT lower(trim(email)) AS normalised, count(*) AS rows
    INTO collision
    FROM users
    GROUP BY lower(trim(email))
    HAVING count(*) > 1
    LIMIT 1;

    IF FOUND THEN
        RAISE EXCEPTION
            'users.email collision on normalised form %: % rows. Merge the duplicates with ''systemprompt admin users merge'' and re-run migrations.',
            collision.normalised, collision.rows;
    END IF;
END $$;

UPDATE users SET email = lower(trim(email)) WHERE email <> lower(trim(email));

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_normalised;
ALTER TABLE users ADD CONSTRAINT users_email_normalised CHECK (email = lower(trim(email)));
