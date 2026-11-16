-- Per table, the composition member whose declarative schema created it,
-- recorded by every schema install. A member that declares a table recorded
-- to another member is refused, whether or not that member is admitted.
CREATE TABLE IF NOT EXISTS schema_table_owners (
    table_name TEXT PRIMARY KEY,
    extension_id TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
