CREATE TABLE IF NOT EXISTS analytics_fact_consumers (
    owner_id TEXT NOT NULL REFERENCES users(id),
    consumer TEXT NOT NULL CHECK(length(consumer) BETWEEN 1 AND 128),
    generation BIGINT NOT NULL DEFAULT 0,
    lease_worker TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0,
    lease_until TIMESTAMPTZ,
    lease_through BIGINT,
    PRIMARY KEY(owner_id,consumer)
);
