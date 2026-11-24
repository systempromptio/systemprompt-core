-- @no-transaction
-- task_execution_steps.status is the StepStatus vocabulary; every writer binds
-- StepStatus's own spelling, so no rewrite precedes the constraint.
-- The constraint is added NOT VALID outside a transaction, so its ACCESS
-- EXCLUSIVE lock is released before VALIDATE scans the rows under SHARE
-- UPDATE EXCLUSIVE, which does not block writers.
ALTER TABLE task_execution_steps DROP CONSTRAINT IF EXISTS task_execution_steps_status_check;
ALTER TABLE task_execution_steps ADD CONSTRAINT task_execution_steps_status_check
    CHECK (status IN ('pending', 'in_progress', 'completed', 'failed')) NOT VALID;
ALTER TABLE task_execution_steps VALIDATE CONSTRAINT task_execution_steps_status_check;
