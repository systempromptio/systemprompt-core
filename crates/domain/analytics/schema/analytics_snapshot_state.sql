CREATE TABLE IF NOT EXISTS analytics_snapshot_state (
 owner_id TEXT PRIMARY KEY REFERENCES users(id), generation BIGINT NOT NULL DEFAULT 0, fact_generation BIGINT NOT NULL DEFAULT 0,
 generated_at TIMESTAMPTZ, day DATE, retained_from DATE, compacted_before DATE, evidence_cutoff TIMESTAMPTZ,
 last_error TEXT
);
