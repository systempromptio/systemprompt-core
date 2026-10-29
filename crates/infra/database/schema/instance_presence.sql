CREATE TABLE IF NOT EXISTS instance_presence (
    instance_id TEXT PRIMARY KEY,
    role TEXT NOT NULL CHECK (role IN ('all', 'gateway', 'api', 'worker', 'runner')),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instance_presence_heartbeat ON instance_presence(heartbeat_at);
