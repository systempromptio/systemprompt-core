CREATE TABLE IF NOT EXISTS managed_publication_selections (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    state TEXT NOT NULL CHECK (state IN ('published','withdrawn')),
    publication_id TEXT NOT NULL,
    revision_id TEXT,
    bundle_digest TEXT CHECK (bundle_digest IS NULL OR bundle_digest ~ '^[0-9a-f]{64}$'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, resource_id),
    FOREIGN KEY(owner_id, resource_id, publication_id) REFERENCES managed_publications(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    CHECK ((state = 'published' AND revision_id IS NOT NULL AND bundle_digest IS NOT NULL)
        OR (state = 'withdrawn' AND revision_id IS NULL AND bundle_digest IS NULL))
);
