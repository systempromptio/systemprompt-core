-- Federated identity mapping: {external_issuer, external_sub} -> local users.id.
--
-- Populated by UserProvider::find_or_create_federated when a token-exchange
-- request from a trusted issuer arrives. Each unique external principal gets
-- exactly one local row; the deterministic primary key prevents double-creation
-- under concurrent first-touch requests for the same (issuer, sub) pair.

CREATE TABLE IF NOT EXISTS federated_identities (
    issuer TEXT NOT NULL,
    external_sub TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (issuer, external_sub)
);

CREATE INDEX IF NOT EXISTS idx_federated_identities_user ON federated_identities (user_id);
