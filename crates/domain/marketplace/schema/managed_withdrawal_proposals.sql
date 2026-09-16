CREATE TABLE IF NOT EXISTS managed_withdrawal_proposals (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (length(reason) BETWEEN 1 AND 4000),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_by TEXT REFERENCES users(id),
    decided_at TIMESTAMPTZ,
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id),
    FOREIGN KEY(owner_id, snapshot_id) REFERENCES managed_source_snapshots(owner_id, id),
    UNIQUE(owner_id, resource_id, snapshot_id)
);
