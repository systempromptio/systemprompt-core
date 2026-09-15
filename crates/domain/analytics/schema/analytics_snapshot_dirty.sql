CREATE TABLE IF NOT EXISTS analytics_snapshot_dirty (
 owner_id TEXT NOT NULL REFERENCES users(id),scope TEXT NOT NULL,PRIMARY KEY(owner_id,scope)
);
