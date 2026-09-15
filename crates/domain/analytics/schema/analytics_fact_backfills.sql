CREATE TABLE IF NOT EXISTS analytics_fact_backfills (
    owner_id TEXT NOT NULL REFERENCES users(id),
    job_id TEXT NOT NULL,
    source TEXT NOT NULL,
    cursor TEXT NOT NULL DEFAULT '',
    generation BIGINT NOT NULL DEFAULT 0,
    pages BIGINT NOT NULL DEFAULT 0,
    facts BIGINT NOT NULL DEFAULT 0,
    complete BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,job_id)
);
