-- An outbox append takes its aggregate sequence from a per-aggregate counter
-- row instead of the outbox's current maximum.
CREATE TABLE IF NOT EXISTS event_outbox_aggregate_seq (
    consumer TEXT NOT NULL,
    aggregate_key TEXT NOT NULL CHECK (aggregate_key <> ''),
    seq BIGINT NOT NULL CHECK (seq > 0),
    PRIMARY KEY (consumer, aggregate_key)
);
