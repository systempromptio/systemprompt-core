CREATE TABLE IF NOT EXISTS managed_distribution_deliveries (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    outbox_id TEXT NOT NULL REFERENCES managed_distribution_outbox(id),
    publication_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    bundle_digest TEXT,
    status TEXT NOT NULL CHECK (status IN ('claimed','distributed','failed')),
    claim_token TEXT NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    error TEXT,
    FOREIGN KEY(owner_id, publication_id) REFERENCES managed_publications(owner_id, id),
    UNIQUE(outbox_id),
    UNIQUE(owner_id, claim_token)
);
