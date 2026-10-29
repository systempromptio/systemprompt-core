-- Instance endpoints (schema/instance_endpoints.sql): the address each replica
-- advertises to its peers, refreshed by the registry heartbeat, so a replica
-- that owns no MCP or agent process can forward to the one that does. The
-- table is the base schema's text.
CREATE TABLE IF NOT EXISTS instance_endpoints (
    instance_id TEXT PRIMARY KEY,
    advertise_url TEXT NOT NULL,
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instance_endpoints_heartbeat ON instance_endpoints(heartbeat_at);
