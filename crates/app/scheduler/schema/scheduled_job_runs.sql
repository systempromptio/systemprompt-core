CREATE SEQUENCE IF NOT EXISTS scheduled_job_run_fencing_seq;

CREATE TABLE IF NOT EXISTS scheduled_job_runs (
    job_name TEXT NOT NULL,
    scheduled_at TIMESTAMPTZ NOT NULL,
    instance_key TEXT NOT NULL,
    claimed_by TEXT NOT NULL,
    fencing_token BIGINT NOT NULL,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMPTZ,
    status TEXT,
    PRIMARY KEY (job_name, scheduled_at, instance_key)
);
