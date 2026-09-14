CREATE TABLE IF NOT EXISTS managed_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    parent_id TEXT,
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    manifest JSONB NOT NULL,
    rationale TEXT NOT NULL CHECK (length(rationale) BETWEEN 1 AND 4000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id, resource_id) REFERENCES managed_resources(owner_id, source_id, id),
    FOREIGN KEY(owner_id, source_id, snapshot_id) REFERENCES managed_source_snapshots(owner_id, source_id, id),
    UNIQUE(owner_id, resource_id, id),
    UNIQUE(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, parent_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(resource_id, digest)
);
DROP TRIGGER IF EXISTS managed_revisions_immutable ON managed_revisions;
CREATE TRIGGER managed_revisions_immutable BEFORE UPDATE ON managed_revisions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
