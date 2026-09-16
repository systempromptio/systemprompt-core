ALTER TABLE managed_inventory_state
    ADD COLUMN IF NOT EXISTS sources JSONB NOT NULL DEFAULT '{}'::JSONB;

ALTER TABLE managed_inventory_observations
    ADD COLUMN IF NOT EXISTS sources JSONB NOT NULL DEFAULT '{}'::JSONB;
