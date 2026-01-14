-- Migration: Add error_message column to agent_tasks table
-- Purpose: Store error messages when tasks fail for better debugging and traceability

ALTER TABLE agent_tasks ADD COLUMN IF NOT EXISTS error_message TEXT;

-- Add index for searching failed tasks with error messages
CREATE INDEX IF NOT EXISTS idx_agent_tasks_error_message ON agent_tasks(error_message) WHERE error_message IS NOT NULL;

COMMENT ON COLUMN agent_tasks.error_message IS 'Error message captured when task fails - used for debugging and trace display';
