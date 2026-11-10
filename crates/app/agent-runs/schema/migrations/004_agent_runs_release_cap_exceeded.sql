-- A run interrupted by shutdown more often than its release cap allows
-- fails with its own error class.
ALTER TABLE agent_runs DROP CONSTRAINT IF EXISTS agent_runs_error_class_check;
ALTER TABLE agent_runs ADD CONSTRAINT agent_runs_error_class_check
    CHECK (error_class IN ('ok', 'budget_exceeded', 'timeout', 'output_invalid', 'hook_denied', 'host_crashed', 'connector_unavailable', 'harness_unavailable', 'release_cap_exceeded')) NOT VALID;
ALTER TABLE agent_runs VALIDATE CONSTRAINT agent_runs_error_class_check;
