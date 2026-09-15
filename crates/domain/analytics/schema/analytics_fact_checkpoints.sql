CREATE TABLE IF NOT EXISTS analytics_fact_checkpoints (
    owner_id TEXT PRIMARY KEY REFERENCES users(id),
    generation BIGINT NOT NULL DEFAULT 0,
    applied_changes BIGINT NOT NULL DEFAULT 0,
    superseded_changes BIGINT NOT NULL DEFAULT 0,
    last_applied_at TIMESTAMPTZ,
    last_recorded_at TIMESTAMPTZ
);
