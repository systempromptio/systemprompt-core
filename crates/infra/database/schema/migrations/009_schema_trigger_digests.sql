-- Trigger digests (schema/schema_trigger_digests.sql): per declarative
-- trigger, the digest of its statements and the definition recorded after a
-- schema phase applied them. The table is the base schema's text.
CREATE TABLE IF NOT EXISTS schema_trigger_digests (
    relation TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    declared TEXT NOT NULL,
    live_definition TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (relation, trigger_name)
);
