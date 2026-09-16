CREATE TABLE IF NOT EXISTS managed_dependency_verifications (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    commit_sha TEXT NOT NULL CHECK (commit_sha ~ '^([0-9a-f]{40}|[0-9a-f]{64})$'),
    bundle_digest TEXT NOT NULL CHECK (bundle_digest ~ '^[0-9a-f]{64}$'),
    manifest JSONB NOT NULL CHECK (jsonb_typeof(manifest) = 'object'),
    verified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(owner_id, revision_id, commit_sha, bundle_digest),
    FOREIGN KEY(owner_id, revision_id) REFERENCES managed_revisions(owner_id, id)
);
DROP TRIGGER IF EXISTS managed_dependency_verifications_immutable ON managed_dependency_verifications;
CREATE TRIGGER managed_dependency_verifications_immutable BEFORE UPDATE ON managed_dependency_verifications FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
