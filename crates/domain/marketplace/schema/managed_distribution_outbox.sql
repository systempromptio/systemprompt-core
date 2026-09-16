CREATE TABLE IF NOT EXISTS managed_distribution_outbox (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    generation BIGINT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, publication_id) REFERENCES managed_publications(owner_id, id),
    UNIQUE(publication_id)
);
