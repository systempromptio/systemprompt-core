-- task_execution_steps.status is the StepStatus vocabulary; every writer binds
-- StepStatus's own spelling, so no rewrite precedes the constraint.
ALTER TABLE task_execution_steps DROP CONSTRAINT IF EXISTS task_execution_steps_status_check;
ALTER TABLE task_execution_steps ADD CONSTRAINT task_execution_steps_status_check
    CHECK (status IN ('pending', 'in_progress', 'completed', 'failed'));
