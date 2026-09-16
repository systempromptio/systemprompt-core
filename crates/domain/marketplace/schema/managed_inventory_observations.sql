CREATE TABLE IF NOT EXISTS managed_inventory_observations (
    owner_id TEXT NOT NULL REFERENCES users(id),
    generation BIGINT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    entries BIGINT NOT NULL,
    PRIMARY KEY(owner_id,generation)
);
CREATE INDEX IF NOT EXISTS managed_inventory_observations_time ON managed_inventory_observations(owner_id,observed_at);
