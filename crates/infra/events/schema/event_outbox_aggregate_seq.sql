-- The last sequence an outbox aggregate was handed, per consumer. An append
-- to an aggregate takes the next one by upserting this row, so two appends
-- to one aggregate serialise on its row lock and never share a sequence, and
-- no append scans the outbox for the current maximum.
CREATE TABLE IF NOT EXISTS event_outbox_aggregate_seq (
    consumer TEXT NOT NULL,
    aggregate_key TEXT NOT NULL CHECK (aggregate_key <> ''),
    seq BIGINT NOT NULL CHECK (seq > 0),
    PRIMARY KEY (consumer, aggregate_key)
);
