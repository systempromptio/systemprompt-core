-- Instance presence (schema/instance_presence.sql): every replica, whatever
-- its `server.role`, records its role with each registry heartbeat, so the
-- live replica count per role is readable without a process registry row.
-- The table is the base schema's text.
CREATE TABLE IF NOT EXISTS instance_presence (
    instance_id TEXT PRIMARY KEY,
    role TEXT NOT NULL CHECK (role IN ('all', 'gateway', 'api', 'worker', 'runner')),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instance_presence_heartbeat ON instance_presence(heartbeat_at);
