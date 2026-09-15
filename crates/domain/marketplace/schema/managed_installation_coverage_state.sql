CREATE TABLE IF NOT EXISTS managed_installation_coverage_state (
 owner_id TEXT PRIMARY KEY REFERENCES users(id),generation BIGINT NOT NULL DEFAULT 0,observed_at TIMESTAMPTZ
);
