CREATE TABLE IF NOT EXISTS analytics_ingestion_producers (
    producer TEXT PRIMARY KEY CHECK (length(producer) BETWEEN 1 AND 180),
    pending_count BIGINT NOT NULL DEFAULT 0 CHECK (pending_count >= 0),
    oldest_pending_at TIMESTAMPTZ,
    checkpoint BIGINT NOT NULL DEFAULT 0 CHECK (checkpoint >= 0),
    last_drained_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (pending_count > 0 OR oldest_pending_at IS NULL)
);
