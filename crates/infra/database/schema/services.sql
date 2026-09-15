CREATE TABLE IF NOT EXISTS services (
    instance_id TEXT NOT NULL,
    name TEXT NOT NULL,
    module_name TEXT NOT NULL,
    server_type TEXT NOT NULL DEFAULT 'internal',
    pid INTEGER,
    port INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'stopped',
    binary_mtime BIGINT,
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (instance_id, name)
);

CREATE INDEX IF NOT EXISTS idx_services_status ON services(status);
CREATE INDEX IF NOT EXISTS idx_services_module ON services(module_name);
CREATE INDEX IF NOT EXISTS idx_services_heartbeat ON services(heartbeat_at);
