-- A run that names a harness records the digest it installed, and a harness
-- that cannot be loaded or verified finishes the run as `harness_unavailable`.
ALTER TABLE agent_runs ADD COLUMN IF NOT EXISTS harness_digest TEXT;

ALTER TABLE agent_runs DROP CONSTRAINT IF EXISTS agent_runs_error_class_check;
ALTER TABLE agent_runs
    ADD CONSTRAINT agent_runs_error_class_check
    CHECK (error_class IN ('ok', 'budget_exceeded', 'timeout', 'output_invalid', 'hook_denied', 'host_crashed', 'connector_unavailable', 'harness_unavailable'))
    NOT VALID;
ALTER TABLE agent_runs VALIDATE CONSTRAINT agent_runs_error_class_check;
