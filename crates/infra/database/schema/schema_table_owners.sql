-- Per table, the composition member whose declarative schema created it,
-- recorded by every schema install. A member that declares a table recorded
-- to another member is refused, whether or not that member is admitted, and
-- an install member claiming a table recorded under core standing is refused
-- even when its id matches.
CREATE TABLE IF NOT EXISTS schema_table_owners (
    table_name TEXT PRIMARY KEY,
    extension_id TEXT NOT NULL,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
