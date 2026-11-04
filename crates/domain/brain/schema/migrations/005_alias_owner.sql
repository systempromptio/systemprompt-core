-- Alias ownership is a rule (the fact with the greatest (updated_at, id)
-- among those that contribute the alias), not an order of arrival: the
-- set reindex records which fact an alias row was written for and when,
-- and only moves an alias to a fact at least as recent as its owner.
-- Columns only; rows written before this stamp carry NULL and yield to the
-- first reindex that touches them.
-- @cost: rows=0 measured=5ms triggers=live

ALTER TABLE brain_aliases
    ADD COLUMN IF NOT EXISTS owner_fact_id TEXT,
    ADD COLUMN IF NOT EXISTS owner_updated_at TIMESTAMPTZ;
