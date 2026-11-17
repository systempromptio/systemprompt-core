-- Per composition member, its user-purge declarations (tables and orphan
-- sweeps, typed JSON) and its composition position, recorded by every schema
-- install in which it is admitted. The user purge runs the recorded
-- declarations of every member absent from its composition. An install member
-- whose id matches a row recorded under core standing is refused.
CREATE TABLE IF NOT EXISTS schema_purge_records (
    extension_id TEXT PRIMARY KEY,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    position INTEGER NOT NULL,
    declarations JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
