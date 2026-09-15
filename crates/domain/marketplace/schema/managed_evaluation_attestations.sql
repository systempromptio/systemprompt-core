CREATE TABLE IF NOT EXISTS managed_evaluation_attestations (
    owner_id TEXT NOT NULL REFERENCES users(id),
    resource_id TEXT NOT NULL REFERENCES managed_resources(id),
    revision_id TEXT NOT NULL REFERENCES managed_revisions(id),
    bundle_digest TEXT NOT NULL,
    experiment_id TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    evidence_digest TEXT NOT NULL,
    source_commit TEXT NOT NULL,
    attested_by TEXT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,resource_id,revision_id,experiment_id),
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id),
    FOREIGN KEY(owner_id,revision_id) REFERENCES managed_revisions(owner_id,id)
);
