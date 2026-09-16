CREATE TABLE IF NOT EXISTS analytics_fact_deltas (
    owner_id TEXT NOT NULL REFERENCES users(id),
    generation BIGINT NOT NULL,
    fact_kind TEXT NOT NULL,
    source TEXT NOT NULL,
    fact_id TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    before_fact JSONB,
    after_fact JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    consumed_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id,generation)
);
CREATE INDEX IF NOT EXISTS analytics_fact_deltas_pending ON analytics_fact_deltas(owner_id,generation) WHERE consumed_at IS NULL;
