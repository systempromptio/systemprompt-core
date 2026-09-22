-- `idx_scheduled_jobs_job_name` has exactly the column of the UNIQUE
-- constraint `scheduled_jobs_job_name_key`, whose index already serves every
-- lookup by job name. It is not UNIQUE and backs no constraint.
DROP INDEX IF EXISTS idx_scheduled_jobs_job_name;
