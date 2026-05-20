CREATE TABLE IF NOT EXISTS federated_identities (
    issuer TEXT NOT NULL,
    external_sub TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (issuer, external_sub)
);
CREATE INDEX IF NOT EXISTS idx_federated_identities_user ON federated_identities (user_id);
