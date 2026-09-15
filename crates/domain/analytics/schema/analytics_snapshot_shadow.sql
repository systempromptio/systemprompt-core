CREATE TABLE IF NOT EXISTS analytics_snapshot_shadow (
 owner_id TEXT NOT NULL REFERENCES users(id), fact_kind TEXT NOT NULL, source TEXT NOT NULL, fact_id TEXT NOT NULL,
 fact JSONB NOT NULL, occurred_at TIMESTAMPTZ NOT NULL, generation BIGINT NOT NULL,
 PRIMARY KEY(owner_id,fact_kind,source,fact_id)
);
CREATE INDEX IF NOT EXISTS analytics_snapshot_shadow_time ON analytics_snapshot_shadow(owner_id,occurred_at);
