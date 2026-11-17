-- Table owners (schema/schema_table_owners.sql): per table, the member
-- whose declarative schema created it. The table is the base schema's text.
CREATE TABLE IF NOT EXISTS schema_table_owners (
    table_name TEXT PRIMARY KEY,
    extension_id TEXT NOT NULL,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
