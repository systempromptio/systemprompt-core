CREATE TABLE IF NOT EXISTS managed_inventory_membership (
    owner_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    record JSONB NOT NULL,
    PRIMARY KEY(owner_id,entry_id,effective_from),
    FOREIGN KEY(owner_id,entry_id) REFERENCES managed_inventory_entries(owner_id,entry_id),
    CHECK(effective_until IS NULL OR effective_until>=effective_from)
);
CREATE UNIQUE INDEX IF NOT EXISTS managed_inventory_open_membership ON managed_inventory_membership(owner_id,entry_id) WHERE effective_until IS NULL;
