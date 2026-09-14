CREATE TABLE IF NOT EXISTS managed_resources (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    upstream_key TEXT NOT NULL CHECK (length(upstream_key) BETWEEN 1 AND 200),
    kind TEXT NOT NULL CHECK (kind IN ('skill','plugin','marketplace','supporting')),
    resource_key TEXT NOT NULL CHECK (length(resource_key) BETWEEN 1 AND 200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id),
    UNIQUE(source_id, upstream_key),
    UNIQUE(owner_id, kind, resource_key),
    UNIQUE(owner_id, id),
    UNIQUE(owner_id, source_id, id)
);
DROP TRIGGER IF EXISTS managed_resources_immutable ON managed_resources;
CREATE TRIGGER managed_resources_immutable BEFORE UPDATE ON managed_resources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
