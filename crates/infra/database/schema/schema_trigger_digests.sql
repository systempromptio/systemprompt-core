-- One row per declarative trigger: the digest of the statements that declare
-- it and the definition Postgres reported right after a schema phase applied
-- them. A later phase leaves the trigger in place, and takes no lock on its
-- table, while both still match.
CREATE TABLE IF NOT EXISTS schema_trigger_digests (
    relation TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    declared TEXT NOT NULL,
    live_definition TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (relation, trigger_name)
);
