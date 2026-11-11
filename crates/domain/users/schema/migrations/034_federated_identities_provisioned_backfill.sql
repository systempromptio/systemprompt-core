-- Records which links that predate the provisioned column were created by
-- the issuer provisioning their account. Provisioning inserted the account
-- and its link in one transaction, so the link's created_at (transaction
-- start, database clock) and the account's created_at (application clock,
-- inside that transaction) fall within seconds of each other. A link to an
-- account that already existed, by email or by an explicit link, was
-- created after that account and stays not provisioned. Links written once
-- the column existed already carry the right value and are left alone.
UPDATE federated_identities f
   SET provisioned = true
  FROM users u
 WHERE u.id = f.user_id
   AND NOT f.provisioned
   AND f.created_at < COALESCE(
           (SELECT m.applied_at FROM extension_migrations m
             WHERE m.extension_id = 'users' AND m.version = 33),
           now())
   AND u.created_at BETWEEN f.created_at - interval '5 seconds'
                        AND f.created_at + interval '30 seconds';
