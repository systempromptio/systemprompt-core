-- User-purge records (schema/schema_purge_records.sql): per member, the
-- purge declarations the user purge runs while that member is disabled.
-- The table is the base schema's text.
CREATE TABLE IF NOT EXISTS schema_purge_records (
    extension_id TEXT PRIMARY KEY,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    position INTEGER NOT NULL,
    declarations JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
