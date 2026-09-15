CREATE TABLE IF NOT EXISTS managed_inventory_state (
    owner_id TEXT PRIMARY KEY REFERENCES users(id),
    generation BIGINT NOT NULL DEFAULT 0,
    observed_at TIMESTAMPTZ,
    entries BIGINT NOT NULL DEFAULT 0,
    last_error TEXT
);
