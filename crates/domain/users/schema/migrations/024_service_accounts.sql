-- Account kind (schema/users.sql): a 'service' account is a non-human
-- principal that workflows and agent runs act as when they write to an
-- external system. It can hold API keys and in-process JWTs but never an
-- interactive credential (session login, passkey, bridge code, OAuth
-- authorization code). Every existing row is a person. Same DDL as
-- schema/users.sql.
ALTER TABLE users ADD COLUMN IF NOT EXISTS account_kind TEXT NOT NULL DEFAULT 'person'
    CONSTRAINT users_account_kind_check CHECK (account_kind IN ('person', 'service'));
CREATE INDEX IF NOT EXISTS idx_users_service_accounts ON users(created_at)
    WHERE account_kind = 'service';

-- Caller purpose (schema/user_api_keys.sql): set by the issuer of a key, for
-- example the workflow a service account's key was minted for, and forwarded
-- to the credential broker. The bearer of the key can never set it.
ALTER TABLE user_api_keys ADD COLUMN IF NOT EXISTS purpose TEXT;
