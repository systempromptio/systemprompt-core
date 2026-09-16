CREATE TABLE IF NOT EXISTS managed_reconciliations (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    upstream_base_revision_id TEXT NOT NULL,
    managed_candidate_revision_id TEXT NOT NULL,
    incoming_revision_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','resolved','withdrawal_proposed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_revision_id TEXT,
    resolved_by TEXT REFERENCES users(id),
    resolved_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, resource_id, upstream_base_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, managed_candidate_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, incoming_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    FOREIGN KEY(owner_id, resource_id, resolved_revision_id) REFERENCES managed_revisions(owner_id, resource_id, id),
    UNIQUE(owner_id, resource_id, managed_candidate_revision_id, incoming_revision_id)
);
