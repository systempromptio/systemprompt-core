ALTER TABLE ai_requests ALTER COLUMN task_id TYPE TEXT;

UPDATE ai_requests
SET task_id = NULL
WHERE task_id IS NOT NULL
  AND task_id NOT IN (SELECT task_id FROM agent_tasks);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_ai_requests_task_id'
          AND table_name = 'ai_requests'
    ) THEN
        ALTER TABLE ai_requests
            ADD CONSTRAINT fk_ai_requests_task_id
            FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id)
            ON DELETE SET NULL;
    END IF;
END $$;
