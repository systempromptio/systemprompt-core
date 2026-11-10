-- A federated link records whether the issuer provisioned the account it
-- points at. Only such accounts take the issuer's roles; a link to an account
-- that existed before the identity arrived never does. Links created before
-- this column existed are recorded as not provisioned.
ALTER TABLE federated_identities
    ADD COLUMN IF NOT EXISTS provisioned BOOLEAN NOT NULL DEFAULT false;
