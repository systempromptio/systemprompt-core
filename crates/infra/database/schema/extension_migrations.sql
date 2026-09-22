CREATE TABLE IF NOT EXISTS extension_migrations (
    id TEXT PRIMARY KEY,
    extension_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(extension_id, version)
);

