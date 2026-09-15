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
-- No UPDATE path exists for immutable provenance or content. Deletion remains
-- available to retention after all references have been removed.
CREATE OR REPLACE FUNCTION reject_managed_content_update() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'managed content is immutable' USING ERRCODE = '23514';
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS managed_sources_immutable ON managed_sources;
CREATE TRIGGER managed_sources_immutable BEFORE UPDATE ON managed_sources FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
