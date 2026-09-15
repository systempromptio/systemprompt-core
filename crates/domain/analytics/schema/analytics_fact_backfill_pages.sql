CREATE TABLE IF NOT EXISTS analytics_fact_backfill_pages (
    owner_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    page_generation BIGINT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(owner_id,job_id,page_generation),
    FOREIGN KEY(owner_id,job_id) REFERENCES analytics_fact_backfills(owner_id,job_id)
);
