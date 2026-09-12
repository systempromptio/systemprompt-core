-- Immutable content and explicit publication: imports never move active revisions.
CREATE TABLE IF NOT EXISTS managed_sources (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    kind TEXT NOT NULL CHECK (kind IN ('git', 'local_tree', 'managed')),
    specification JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(owner_id, name),
    UNIQUE(owner_id, id)
);
CREATE TABLE IF NOT EXISTS managed_source_snapshots (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    provenance JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, source_id) REFERENCES managed_sources(owner_id, id),
    UNIQUE(owner_id, source_id, digest),
    UNIQUE(owner_id, source_id, id)
);
CREATE TABLE IF NOT EXISTS managed_assets (
    owner_id TEXT NOT NULL REFERENCES users(id),
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL CHECK (octet_length(content) <= 16777216),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, digest)
);
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
    UNIQUE(kind, resource_key),
    UNIQUE(owner_id, id),
    UNIQUE(owner_id, source_id, id)
);
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
CREATE TABLE IF NOT EXISTS managed_revision_assets (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    path TEXT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(revision_id, path),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, digest) REFERENCES managed_assets(owner_id, digest)
);
CREATE TABLE IF NOT EXISTS managed_revision_dependencies (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    dependency_id TEXT NOT NULL,
    PRIMARY KEY(revision_id, dependency_id),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, dependency_id) REFERENCES managed_revisions(owner_id, id),
    CHECK (revision_id <> dependency_id)
);
-- No UPDATE path exists for immutable provenance or content. Deletion remains
-- available to retention after all references have been removed.
CREATE OR REPLACE FUNCTION reject_managed_content_update() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'managed content is immutable' USING ERRCODE = '23514';
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS managed_sources_immutable ON managed_sources;
CREATE TRIGGER managed_sources_immutable BEFORE UPDATE ON managed_sources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_snapshots_immutable ON managed_source_snapshots;
CREATE TRIGGER managed_snapshots_immutable BEFORE UPDATE ON managed_source_snapshots FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_assets_immutable ON managed_assets;
CREATE TRIGGER managed_assets_immutable BEFORE UPDATE ON managed_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_resources_immutable ON managed_resources;
CREATE TRIGGER managed_resources_immutable BEFORE UPDATE ON managed_resources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_revisions_immutable ON managed_revisions;
CREATE TRIGGER managed_revisions_immutable BEFORE UPDATE ON managed_revisions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_revision_assets_immutable ON managed_revision_assets;
CREATE TRIGGER managed_revision_assets_immutable BEFORE UPDATE ON managed_revision_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_dependencies_immutable ON managed_revision_dependencies;
CREATE TRIGGER managed_dependencies_immutable BEFORE UPDATE ON managed_revision_dependencies FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
