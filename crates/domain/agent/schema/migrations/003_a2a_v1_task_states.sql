BEGIN;

UPDATE agent_tasks SET status = 'TASK_STATE_SUBMITTED' WHERE status = 'submitted';
UPDATE agent_tasks SET status = 'TASK_STATE_WORKING' WHERE status = 'working';
UPDATE agent_tasks SET status = 'TASK_STATE_INPUT_REQUIRED' WHERE status = 'input-required';
UPDATE agent_tasks SET status = 'TASK_STATE_COMPLETED' WHERE status = 'completed';
UPDATE agent_tasks SET status = 'TASK_STATE_CANCELED' WHERE status = 'canceled';
UPDATE agent_tasks SET status = 'TASK_STATE_FAILED' WHERE status = 'failed';
UPDATE agent_tasks SET status = 'TASK_STATE_REJECTED' WHERE status = 'rejected';
UPDATE agent_tasks SET status = 'TASK_STATE_AUTH_REQUIRED' WHERE status = 'auth-required';
UPDATE agent_tasks SET status = 'TASK_STATE_UNKNOWN' WHERE status = 'unknown';

ALTER TABLE agent_tasks DROP CONSTRAINT IF EXISTS agent_tasks_status_check;
ALTER TABLE agent_tasks ADD CONSTRAINT agent_tasks_status_check CHECK (
    status IN (
        'TASK_STATE_PENDING', 'TASK_STATE_SUBMITTED', 'TASK_STATE_WORKING',
        'TASK_STATE_INPUT_REQUIRED', 'TASK_STATE_COMPLETED', 'TASK_STATE_CANCELED',
        'TASK_STATE_FAILED', 'TASK_STATE_REJECTED', 'TASK_STATE_AUTH_REQUIRED',
        'TASK_STATE_UNKNOWN'
    )
);

ALTER TABLE agent_tasks ALTER COLUMN status SET DEFAULT 'TASK_STATE_SUBMITTED';

COMMIT;
