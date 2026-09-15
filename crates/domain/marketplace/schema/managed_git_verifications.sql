CREATE TABLE IF NOT EXISTS managed_git_verifications (
    owner_id TEXT NOT NULL,
    revision_id TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    source_id TEXT NOT NULL,
    upstream_root TEXT NOT NULL,
    bundle_digest TEXT NOT NULL,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner_id, revision_id, commit_sha),
    FOREIGN KEY (owner_id, revision_id) REFERENCES managed_revisions(owner_id, id),
    FOREIGN KEY (owner_id, source_id) REFERENCES managed_sources(owner_id, id)
);
