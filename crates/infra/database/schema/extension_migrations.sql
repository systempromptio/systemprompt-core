CREATE TABLE IF NOT EXISTS extension_migrations (
    id TEXT PRIMARY KEY,
    extension_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(extension_id, version)
);

CREATE INDEX IF NOT EXISTS idx_extension_migrations_ext_id ON extension_migrations(extension_id);
CREATE INDEX IF NOT EXISTS idx_extension_migrations_ext_version ON extension_migrations(extension_id, version);
