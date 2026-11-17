-- Per index, routine, view, materialized view, sequence, type, domain and
-- trigger, the composition member that owns it, recorded by every schema
-- install. A member absent from the composition keeps its objects: another
-- member's statement that creates, alters or drops one is judged against
-- this record. A trigger is named '<trigger> ON <schema>.<table>'.
CREATE TABLE IF NOT EXISTS schema_object_owners (
    object_kind TEXT NOT NULL CHECK (object_kind IN ('object', 'index', 'trigger')),
    object_name TEXT NOT NULL,
    extension_id TEXT NOT NULL,
    standing TEXT NOT NULL CHECK (standing IN ('core', 'install')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (object_kind, object_name)
);
