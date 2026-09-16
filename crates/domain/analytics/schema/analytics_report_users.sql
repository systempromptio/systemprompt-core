CREATE TABLE IF NOT EXISTS analytics_report_users (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    status TEXT NOT NULL,
    roles TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_users_created_at ON analytics_report_users (created_at);
