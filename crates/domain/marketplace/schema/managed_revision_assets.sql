CREATE TABLE IF NOT EXISTS managed_revision_assets (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    path TEXT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(revision_id, path),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, digest) REFERENCES managed_assets(owner_id, digest)
);
DROP TRIGGER IF EXISTS managed_revision_assets_immutable ON managed_revision_assets;
CREATE TRIGGER managed_revision_assets_immutable BEFORE UPDATE ON managed_revision_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
