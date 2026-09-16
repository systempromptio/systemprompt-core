CREATE TABLE IF NOT EXISTS managed_revision_dependencies (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    dependency_id TEXT NOT NULL,
    PRIMARY KEY(revision_id, dependency_id),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY(owner_id, dependency_id) REFERENCES managed_revisions(owner_id, id),
    CHECK (revision_id <> dependency_id)
);
DROP TRIGGER IF EXISTS managed_dependencies_immutable ON managed_revision_dependencies;
CREATE TRIGGER managed_dependencies_immutable BEFORE UPDATE ON managed_revision_dependencies FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
