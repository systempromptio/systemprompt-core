ALTER TABLE eval_executions ADD COLUMN IF NOT EXISTS deadline_at TIMESTAMPTZ;
UPDATE eval_executions SET deadline_at=NOW() WHERE status='running' AND deadline_at IS NULL;
