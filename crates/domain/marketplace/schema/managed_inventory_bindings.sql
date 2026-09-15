CREATE TABLE IF NOT EXISTS managed_inventory_bindings (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    bound_by TEXT NOT NULL REFERENCES users(id),
    bound_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,entry_id),
    UNIQUE(owner_id,resource_id),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    FOREIGN KEY(owner_id,resource_id) REFERENCES managed_resources(owner_id,id)
);
