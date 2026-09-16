CREATE TABLE IF NOT EXISTS managed_publications (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    action TEXT NOT NULL CHECK (action IN ('initial_adoption','publish_improvement','withdraw','rollback')),
    revision_id TEXT,
    bundle_digest TEXT CHECK (bundle_digest IS NULL OR bundle_digest ~ '^[0-9a-f]{64}$'),
    operation_key TEXT NOT NULL CHECK (length(operation_key) BETWEEN 1 AND 200),
    request_digest TEXT NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, review_id) REFERENCES managed_publication_reviews(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, resource_id, generation),
    UNIQUE(owner_id, operation_key),
    UNIQUE(owner_id, id),
    UNIQUE(owner_id, resource_id, id)
);
DROP TRIGGER IF EXISTS managed_publications_immutable ON managed_publications;
CREATE TRIGGER managed_publications_immutable BEFORE UPDATE ON managed_publications FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
