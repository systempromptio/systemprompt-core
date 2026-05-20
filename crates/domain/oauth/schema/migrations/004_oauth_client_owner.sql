-- Add a required owner for every OAuth client. Existing clients are dropped
-- in the same transaction because client_credentials issuance now mints JWTs
-- as the owner; legacy ownerless clients have no safe interpretation.
BEGIN;

DELETE FROM oauth_clients;

ALTER TABLE oauth_clients
    ADD COLUMN owner_user_id TEXT NOT NULL
    REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX idx_oauth_clients_owner_user_id ON oauth_clients(owner_user_id);

COMMIT;
