CREATE TABLE IF NOT EXISTS analytics_snapshot_daily (
 owner_id TEXT NOT NULL REFERENCES users(id), scope TEXT NOT NULL, day DATE NOT NULL,
 metrics JSONB NOT NULL, spend JSONB NOT NULL, histogram JSONB NOT NULL, histogram_version INTEGER NOT NULL DEFAULT 1,
 cohort BIGINT NOT NULL, suppressed BOOLEAN NOT NULL DEFAULT false, generation BIGINT NOT NULL,
 PRIMARY KEY(owner_id,scope,day)
);
