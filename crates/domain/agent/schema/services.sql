CREATE TABLE IF NOT EXISTS services (
    name TEXT PRIMARY KEY,
    module_name TEXT NOT NULL,
    pid INTEGER,
    port INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'stopped',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_services_status ON services(status);
CREATE INDEX IF NOT EXISTS idx_services_module ON services(module_name);
