-- Scheduled job runs (schema/scheduled_job_runs.sql): one row per job and
-- scheduled instant (and, for a node-scoped job, per instance), inserted by the
-- claiming statement itself, so two replicas ticking for one instant yield one
-- run whatever their clocks read. instance_key is '' for a cluster-scoped job.
-- Each claim carries a token from one sequence; the run's completion is fenced
-- on it. The table is the base schema's text.
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
