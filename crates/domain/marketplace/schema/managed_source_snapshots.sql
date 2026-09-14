CREATE TABLE IF NOT EXISTS managed_source_snapshots (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    provenance JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id),
    UNIQUE(owner_id, source_id, id),
    UNIQUE(owner_id, id)
);
DROP TRIGGER IF EXISTS managed_snapshots_immutable ON managed_source_snapshots;
CREATE TRIGGER managed_snapshots_immutable BEFORE UPDATE ON managed_source_snapshots FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
