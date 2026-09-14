CREATE INDEX IF NOT EXISTS managed_revision_history
    ON managed_revisions(owner_id, resource_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS managed_revision_latest
    ON managed_revisions(resource_id, created_at DESC, id DESC);
