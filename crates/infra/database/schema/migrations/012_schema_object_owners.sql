-- Object owners (schema/schema_object_owners.sql): per non-table object,
-- the member that owns it. The table is the base schema's text.
CREATE TABLE IF NOT EXISTS schema_object_owners (
    object_kind TEXT NOT NULL CHECK (object_kind IN ('object', 'index', 'trigger')),
    object_name TEXT NOT NULL,
    extension_id TEXT NOT NULL,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (object_kind, object_name)
);
