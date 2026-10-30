CREATE TABLE IF NOT EXISTS document_collab_owners (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    owner_id UUID NOT NULL,
    address TEXT NOT NULL,
    lease_until TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
