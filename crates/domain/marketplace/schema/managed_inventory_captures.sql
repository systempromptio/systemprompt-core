CREATE TABLE IF NOT EXISTS managed_inventory_captures (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    status TEXT NOT NULL,
    revision_id TEXT,
    reconciliation_id TEXT,
    diagnostic TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,entry_id,operation_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,revision_id) REFERENCES managed_revisions(owner_id,id)
);
