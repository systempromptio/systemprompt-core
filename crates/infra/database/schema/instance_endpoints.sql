CREATE TABLE IF NOT EXISTS instance_endpoints (
    instance_id TEXT PRIMARY KEY,
    advertise_url TEXT NOT NULL,
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instance_endpoints_heartbeat ON instance_endpoints(heartbeat_at);
