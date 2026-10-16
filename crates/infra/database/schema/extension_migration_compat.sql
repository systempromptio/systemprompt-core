CREATE TABLE IF NOT EXISTS extension_migration_compat (
    extension_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    breaks_older BOOLEAN NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (extension_id, version)
);
